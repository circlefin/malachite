//! Network state management

use std::collections::{HashMap, HashSet};
use std::fmt;

use libp2p::identify;
use libp2p::request_response::InboundRequestId;
use libp2p::Multiaddr;
use malachitebft_discovery as discovery;
use malachitebft_sync as sync;

use crate::behaviour::Behaviour;
use crate::metrics::Metrics as NetworkMetrics;
use crate::{Channel, ChannelNames, PeerType};
use malachitebft_discovery::ConnectionDirection;

/// Public network state dump for external consumers
#[derive(Clone, Debug)]
pub struct NetworkStateDump {
    pub local_node: LocalNodeInfo,
    pub peers: std::collections::HashMap<libp2p::PeerId, PeerInfo>,
    pub validator_set: Vec<ValidatorInfo>,
    pub persistent_peer_ids: Vec<libp2p::PeerId>,
    pub persistent_peer_addrs: Vec<Multiaddr>,
}

/// Validator information passed from consensus to network layer
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValidatorInfo {
    /// Consensus address as string (for matching via Identify protocol)
    pub address: String,
    /// Voting power
    pub voting_power: u64,
}

/// Local node information
#[derive(Clone, Debug)]
pub struct LocalNodeInfo {
    pub moniker: String,
    pub peer_id: libp2p::PeerId,
    pub listen_addr: Multiaddr,
    /// This node's consensus address (if it is configured with validator credentials).
    ///
    /// Present if the node has a consensus keypair, even if not currently in the active validator set.
    /// This is static configuration determined at startup.
    /// Note: In the future full nodes will not have a consensus address, so this will be None.
    pub consensus_address: Option<String>,
    /// Whether this node is currently in the active validator set.
    ///
    /// Updated dynamically when validator set changes. A node can have `consensus_address = Some(...)`
    /// but `is_validator = false` if it was removed from the validator set or hasn't joined yet.
    pub is_validator: bool,
    /// Whether this node only accepts connections from persistent peers.
    pub persistent_peers_only: bool,
    pub subscribed_topics: HashSet<String>,
}

impl fmt::Display for LocalNodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut topics: Vec<&str> = self.subscribed_topics.iter().map(|s| s.as_str()).collect();
        topics.sort();
        let topics_str = format!("[{}]", topics.join(","));
        let address = self.consensus_address.as_deref().unwrap_or("none");
        let role = if self.is_validator {
            "validator"
        } else {
            "full_node"
        };
        let peers_mode = if self.persistent_peers_only {
            "persistent_only"
        } else {
            "open"
        };
        write!(
            f,
            "{}, {}, {}, {}, {}, {}, {}, me",
            self.listen_addr, self.moniker, role, self.peer_id, address, topics_str, peers_mode
        )
    }
}

/// Peer information without slot number (for State, which has no cardinality limits)
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub address: Multiaddr,
    pub consensus_address: String, // Consensus address as string (for validator matching)
    pub moniker: String,
    pub peer_type: PeerType,
    pub connection_direction: Option<ConnectionDirection>, // None if ephemeral (unknown)
    pub score: f64,
    pub topics: HashSet<String>, // Set of topics peer is in mesh for (e.g., "/consensus", "/liveness")
}

impl PeerInfo {
    /// Format peer info with peer_id for logging
    ///  Address, Moniker, Type, PeerId, ConsensusAddr, Mesh, Dir, Score
    pub fn format_with_peer_id(&self, peer_id: &libp2p::PeerId) -> String {
        let direction = self.connection_direction.map_or("??", |d| d.as_str());
        let mut topics: Vec<&str> = self.topics.iter().map(|s| s.as_str()).collect();
        topics.sort();
        let topics_str = format!("[{}]", topics.join(","));
        let peer_type_str = self.peer_type.primary_type_str();
        let address = if self.consensus_address.is_empty() {
            "none"
        } else {
            &self.consensus_address
        };
        format!(
            "{}, {}, {}, {}, {}, {}, {}, {}",
            self.address,
            self.moniker,
            peer_type_str,
            peer_id,
            address,
            topics_str,
            direction,
            self.score as i64
        )
    }
}

#[derive(Debug)]
pub struct State {
    pub sync_channels: HashMap<InboundRequestId, sync::ResponseChannel>,
    pub discovery: discovery::Discovery<Behaviour>,
    pub persistent_peer_ids: HashSet<libp2p::PeerId>,
    pub persistent_peer_addrs: Vec<Multiaddr>,
    /// Latest validator set from consensus
    pub validator_set: Vec<ValidatorInfo>,
    pub(crate) metrics: NetworkMetrics,
    /// Local node information
    pub local_node: LocalNodeInfo,
    /// Detailed peer information indexed by PeerId (for RPC queries and metrics)
    pub peer_info: HashMap<libp2p::PeerId, PeerInfo>,
}

impl State {
    /// Process a validator set update from consensus.
    ///
    /// This method:
    /// - Updates the validator set
    /// - Updates local node validator status and metrics
    /// - Re-classifies all connected peers based on the new validator set
    ///
    /// Returns a list of (peer_id, new_score) for peers whose type changed,
    /// so the caller can update GossipSub scores.
    pub(crate) fn process_validator_set_update(
        &mut self,
        new_validators: Vec<ValidatorInfo>,
    ) -> Vec<(libp2p::PeerId, f64)> {
        // Store the new validator set
        self.validator_set = new_validators;

        self.reclassify_local_node();

        // Re-classify all connected peers
        self.reclassify_peers()
    }

    /// Re-classify the local node based on the current validator set.
    fn reclassify_local_node(&mut self) {
        let was_validator = self.local_node.is_validator;
        // Update local node status
        let local_is_validator = self
            .local_node
            .consensus_address
            .as_ref()
            .map(|addr| self.validator_set.iter().any(|v| &v.address == addr))
            .unwrap_or(false);

        self.local_node.is_validator = local_is_validator;

        // Log and update metrics for local node status change
        if was_validator != local_is_validator {
            tracing::info!(
                local_is_validator,
                address = ?self.local_node.consensus_address,
                "Local node validator status changed"
            );
            self.metrics.set_local_node_info(&self.local_node);
        }
    }

    /// Re-classify all connected peers based on the current validator set.
    ///
    /// Returns a list of (peer_id, new_score) for peers whose type changed.
    fn reclassify_peers(&mut self) -> Vec<(libp2p::PeerId, f64)> {
        let mut changed_peers = Vec::new();

        for (peer_id, peer_info) in self.peer_info.iter_mut() {
            let old_type = peer_info.peer_type;

            // Check if advertised address matches a validator in the set
            // If it does, use the canonical address from the validator set
            let is_validator = if let Some(validator_info) = self
                .validator_set
                .iter()
                .find(|v| v.address == peer_info.consensus_address)
            {
                // Use canonical address from validator set
                peer_info.consensus_address = validator_info.address.clone();
                true
            } else {
                false
            };

            // Preserve persistent status, update validator status
            let new_type = peer_info.peer_type.with_validator_status(is_validator);

            if new_type != old_type {
                tracing::info!(
                    %peer_id,
                    ?old_type,
                    ?new_type,
                    "Peer type changed due to validator set update"
                );

                // Clone peer_info before updating for metrics (need old state)
                let old_peer_info = peer_info.clone();

                // Compute new score
                let new_score = crate::peer_scoring::get_peer_score(new_type);

                // Update peer type and score
                peer_info.peer_type = new_type;
                peer_info.score = new_score;

                // Update metrics with old info and new type
                self.metrics
                    .update_peer_type(peer_id, &old_peer_info, new_type);

                // Record for caller to update GossipSub scores
                changed_peers.push((*peer_id, new_score));
            }
        }

        changed_peers
    }

    pub(crate) fn new(
        discovery: discovery::Discovery<Behaviour>,
        persistent_peer_addrs: Vec<Multiaddr>,
        local_node: LocalNodeInfo,
        metrics: NetworkMetrics,
    ) -> Self {
        // Extract PeerIds from persistent peer Multiaddrs if they contain /p2p/<peer_id>
        let persistent_peer_ids = persistent_peer_addrs
            .iter()
            .filter_map(extract_peer_id_from_multiaddr)
            .collect();

        Self {
            sync_channels: Default::default(),
            discovery,
            persistent_peer_ids,
            persistent_peer_addrs,
            validator_set: Vec::new(),
            metrics,
            local_node,
            peer_info: HashMap::new(),
        }
    }

    /// Determine the peer type based on peer ID and identify info
    pub(crate) fn peer_type(
        &self,
        peer_id: &libp2p::PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        info: &identify::Info,
    ) -> PeerType {
        let is_persistent = self.persistent_peer_ids.contains(peer_id)
            || self.is_persistent_peer_by_address(connection_id);

        // Extract validator address from agent_version and check if it's in the validator set
        let agent_info = crate::utils::parse_agent_version(&info.agent_version);
        let is_validator = agent_info.address != "unknown"
            && self
                .validator_set
                .iter()
                .any(|v| v.address == agent_info.address);

        PeerType::new(is_persistent, is_validator)
    }

    /// Check if a peer is a persistent peer by matching its addresses against persistent peer addresses
    ///
    /// For inbound connections, we use the actual remote address from the connection endpoint
    /// to prevent address spoofing attacks where a malicious peer could claim to be a
    /// persistent peer by faking its `listen_addrs` in the Identify message.
    fn is_persistent_peer_by_address(&self, connection_id: libp2p::swarm::ConnectionId) -> bool {
        // Use actual remote address for both inbound and outbound connections
        // This prevents address spoofing for inbound, and for outbound it's the address we dialed
        let Some(conn_info) = self.discovery.connections.get(&connection_id) else {
            return false;
        };

        let remote_addr_without_p2p = strip_peer_id_from_multiaddr(&conn_info.remote_addr);

        for persistent_addr in &self.persistent_peer_addrs {
            let persistent_addr_without_p2p = strip_peer_id_from_multiaddr(persistent_addr);

            if remote_addr_without_p2p == persistent_addr_without_p2p {
                return true;
            }
        }

        false
    }

    /// Update peer information from gossipsub (scores and mesh membership)
    /// Also updates metrics based on the updated State
    pub(crate) fn update_peer_info(
        &mut self,
        gossipsub: &libp2p_gossipsub::Behaviour,
        channels: &[Channel],
        channel_names: ChannelNames,
    ) {
        // Clean up disconnected peers from State
        let current_peers: HashSet<libp2p::PeerId> =
            gossipsub.all_peers().map(|(p, _)| *p).collect();
        let tracked_peers: HashSet<libp2p::PeerId> = self.peer_info.keys().copied().collect();
        let disconnected_peers: Vec<libp2p::PeerId> =
            tracked_peers.difference(&current_peers).copied().collect();

        for peer_id in disconnected_peers {
            // Remove from State
            if let Some(peer_info) = self.peer_info.remove(&peer_id) {
                // Also free metric slot if peer has one
                self.metrics.free_slot(&peer_id, &peer_info);
            }
        }

        // Build a map of peer_id to the set of topics they're in
        let mut peer_topics: HashMap<libp2p::PeerId, HashSet<String>> = HashMap::new();

        for channel in channels {
            let topic = channel.to_gossipsub_topic(channel_names);
            let topic_hash = topic.hash();
            let topic_str = channel.as_str(channel_names).to_string();

            for peer_id in gossipsub.mesh_peers(&topic_hash) {
                peer_topics
                    .entry(*peer_id)
                    .or_default()
                    .insert(topic_str.clone());
            }
        }

        // Update score and topics for all peers in State
        for (peer_id, peer_info) in self.peer_info.iter_mut() {
            let new_score = gossipsub.peer_score(peer_id).unwrap_or(0.0);
            let new_topics = peer_topics.get(peer_id).cloned().unwrap_or_default();

            // Update metrics before updating peer_info.topics
            // (metrics needs to compare old vs new topics)
            let _ = self.metrics.update_peer_metrics(
                peer_id,
                peer_info,
                new_score,
                Some(new_topics.clone()),
            );

            // Now update peer information in State
            peer_info.score = new_score;
            peer_info.topics = new_topics;
        }
    }

    /// Update the peer information after Identify completes and compute peer score.
    ///
    /// This method:
    /// - Determines the peer type (validator, persistent, etc.)
    /// - Records peer info in state and metrics
    /// - Computes the GossipSub score
    ///
    /// Returns the score to set on the peer in GossipSub.
    pub(crate) fn update_peer(
        &mut self,
        peer_id: libp2p::PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        info: &identify::Info,
    ) -> f64 {
        // Determine peer type using actual remote address for inbound connections
        let peer_type = self.peer_type(&peer_id, connection_id, info);

        // Track persistent peers
        if peer_type.is_persistent() {
            self.persistent_peer_ids.insert(peer_id);
        }

        // Determine peer type direction from discovery layer
        let connection_direction = if self.discovery.is_outbound_peer(&peer_id) {
            Some(ConnectionDirection::Outbound)
        } else if self.discovery.is_inbound_peer(&peer_id) {
            Some(ConnectionDirection::Inbound)
        } else {
            // ephemeral connection (not tracked, will be closed after timeout)
            None
        };

        // Extract address from identify info (use first listen address or placeholder)
        // TODO: filter out unreachable addresses (?)
        let address = info
            .listen_addrs
            .first()
            .cloned()
            .unwrap_or_else(|| "/ip4/0.0.0.0/tcp/0".parse().expect("valid multiaddr"));

        // Parse agent_version to extract moniker and consensus address
        let agent_info = crate::utils::parse_agent_version(&info.agent_version);

        // TODO: The advertised address in agent_version is untrusted, any peer can claim any address.
        // A malicious peer could impersonate a validator by advertising their address.
        // Fix: Require peers to sign their libp2p PeerID with their consensus key to prove ownership.
        let consensus_address = if peer_type.is_validator() {
            // Use canonical address from validator set
            self.validator_set
                .iter()
                .find(|v| v.address == agent_info.address)
                .map(|v| v.address.clone())
                .unwrap_or_else(|| agent_info.address.clone())
        } else {
            agent_info.address.clone()
        };

        // Compute the peer score based on peer type
        let score = crate::peer_scoring::get_peer_score(peer_type);

        // Record peer information in State
        let peer_info = PeerInfo {
            address,
            consensus_address,
            moniker: agent_info.moniker,
            peer_type,
            connection_direction,
            score,
            topics: Default::default(), // Empty set, will be updated by update_peer_info
        };

        // Record peer information in metrics (subject to 100 slot limit)
        self.metrics.record_peer_info(&peer_id, &peer_info);

        // Store in State
        self.peer_info.insert(peer_id, peer_info);

        score
    }

    /// Format the peer information for logging (scrapable format):
    ///  Address, Moniker, PeerId, Mesh, Dir, Type, Score
    pub fn format_peer_info(&self) -> String {
        let mut lines = Vec::new();

        // Header
        lines.push("Address, Moniker, Type, PeerId, ConsensusAddr, Mesh, Dir, Score".to_string());

        // Local node info marked with "me"
        lines.push(format!("{}", self.local_node));

        // Sort peers by moniker
        let mut peers: Vec<_> = self.peer_info.iter().collect();
        peers.sort_by(|a, b| a.1.moniker.cmp(&b.1.moniker));

        for (peer_id, peer_info) in peers {
            lines.push(peer_info.format_with_peer_id(peer_id));
        }

        lines.join("\n")
    }
}

/// Extract PeerId from a Multiaddr if it contains a /p2p/<peer_id> component
fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::multiaddr::Protocol;

    for protocol in addr.iter() {
        if let Protocol::P2p(peer_id) = protocol {
            return Some(peer_id);
        }
    }
    None
}

/// Strip /p2p/<peer_id> component from a Multiaddr for comparison
fn strip_peer_id_from_multiaddr(addr: &Multiaddr) -> Multiaddr {
    use libp2p::multiaddr::Protocol;

    let mut result = Multiaddr::empty();
    for protocol in addr.iter() {
        if !matches!(protocol, Protocol::P2p(_)) {
            result.push(protocol);
        }
    }
    result
}

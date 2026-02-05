//! Network state management

use std::collections::{HashMap, HashSet};
use std::fmt;

use libp2p::identify;
use libp2p::request_response::InboundRequestId;
use libp2p::Multiaddr;
use malachitebft_discovery as discovery;
use malachitebft_discovery::util::strip_peer_id_from_multiaddr;
use malachitebft_sync as sync;

use crate::behaviour::Behaviour;
use crate::metrics::Metrics as NetworkMetrics;
use crate::{Channel, ChannelNames, PeerType, PersistentPeerError};
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
    /// Public key bytes (for matching validator proofs)
    pub public_key: Vec<u8>,
    /// Voting power
    pub voting_power: u64,
}

impl ValidatorInfo {
    /// Returns the address if the public key matches, None otherwise.
    /// Used to look up the address when we only have public key bytes (from proof).
    pub fn address_for_public_key(&self, public_key: &[u8]) -> Option<&str> {
        if self.public_key == public_key {
            Some(&self.address)
        } else {
            None
        }
    }
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
    /// Note: In the future full nodes may not have a consensus address, so this will be None.
    pub consensus_address: Option<String>,
    /// Pre-signed validator proof bytes (if this node has validator credentials).
    ///
    /// Used for the validator proof protocol to prove validator identity.
    pub proof_bytes: Option<bytes::Bytes>,
    /// Whether this node is currently in the active validator set.
    ///
    /// Updated dynamically when validator set changes. A node can have `consensus_address = Some(...)`
    /// but `is_validator = false` if it was removed from the validator set or hasn't joined yet.
    pub is_validator: bool,
    /// Whether this node only accepts connections from persistent peers.
    pub persistent_peers_only: bool,
    /// Set of topics this node is subscribed to
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
    pub moniker: String,
    /// Peer address
    pub address: Multiaddr,
    /// Consensus address, set when peer has a verified proof AND is in the validator set.
    /// Derived from the matching validator in the set.
    /// Used for display/metrics (shorter than raw public key).
    pub consensus_address: Option<String>,
    /// Consensus public key from a verified validator proof.
    /// Set when a valid proof is received, regardless of validator set membership.
    /// Used to re-evaluate validator status when the validator set changes.
    pub consensus_public_key: Option<Vec<u8>>,
    /// Peer type (validator, persistent, full node)
    pub peer_type: PeerType,
    /// Connection direction (outbound or inbound), None if ephemeral
    pub connection_direction: Option<ConnectionDirection>,
    /// GossipSub score
    pub score: f64,
    /// Set of topics peer is in mesh for (e.g., "/consensus", "/liveness")
    pub topics: HashSet<String>,
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
        let address = self.consensus_address.as_deref().unwrap_or("none");
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
    pub validator_set: HashSet<ValidatorInfo>,
    pub(crate) metrics: NetworkMetrics,
    /// Local node information
    pub local_node: LocalNodeInfo,
    /// Detailed peer information indexed by PeerId
    pub peer_info: HashMap<libp2p::PeerId, PeerInfo>,
    /// Pending verified proofs for peers not yet in peer_info (Identify not received yet).
    ///
    /// rust-libp2p does not guarantee Identify runs before other protocols:
    /// <https://docs.rs/libp2p/latest/libp2p/identify/index.html#important-discrepancies>
    ///
    /// If proof verification completes before Identify, we buffer the public_key here
    /// and apply it when Identify completes and creates the PeerInfo.
    pub(crate) pending_verified_proofs: HashMap<libp2p::PeerId, Vec<u8>>,
}

impl State {
    /// Process a validator set update from consensus.
    ///
    /// This method:
    /// - Updates the validator set
    /// - Updates local node validator status and metrics
    /// - Removes peers whose validators were removed from the set
    /// - Promotes peers whose validators were added back to the set
    ///
    /// Returns a list of (peer_id, new_score) for peers whose type changed,
    /// so the caller can update GossipSub scores.
    pub(crate) fn process_validator_set_update(
        &mut self,
        new_validators: HashSet<ValidatorInfo>,
    ) -> Vec<(libp2p::PeerId, f64)> {
        // Store the new validator set
        self.validator_set = new_validators;

        self.reclassify_local_node();

        // Reclassify peers based on stored proofs against new validator set
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

    /// Reclassify peers based on validator set changes.
    ///
    /// For peers with stored proofs (consensus_public_key), re-evaluates validator status
    /// by checking if their public key matches a validator in the new set.
    /// Updates consensus_address accordingly (set if in set, cleared if not).
    ///
    /// Returns a list of (peer_id, new_score) for peers whose type changed.
    fn reclassify_peers(&mut self) -> Vec<(libp2p::PeerId, f64)> {
        let mut changed_peers = Vec::new();

        for (peer_id, peer_info) in self.peer_info.iter_mut() {
            // Only re-evaluate peers with verified proofs
            let Some(public_key) = &peer_info.consensus_public_key else {
                continue;
            };

            // Look up validator by public key to check membership and get address
            let validator_address = self
                .validator_set
                .iter()
                .find_map(|v| v.address_for_public_key(public_key));

            let is_in_validator_set = validator_address.is_some();

            let new_type = peer_info
                .peer_type
                .with_validator_status(is_in_validator_set);

            // Clone old info for metrics BEFORE updating fields
            let old_peer_info = peer_info.clone();

            // Update consensus_address: set if in validator set, clear if not
            peer_info.consensus_address = validator_address.map(|s| s.to_string());

            if let Some(new_score) = apply_peer_type_change(
                peer_id,
                peer_info,
                &old_peer_info,
                new_type,
                &mut self.metrics,
            ) {
                changed_peers.push((*peer_id, new_score));
            }
        }

        changed_peers
    }

    /// Record that a peer sent a valid proof with the given public key.
    ///
    /// The proof's signature has already been verified by the engine. This:
    /// - Stores the public_key (for future validator set matching)
    /// - Sets consensus_address if peer is currently in validator set
    /// - Updates peer_type based on validator set membership
    ///
    /// If the peer is not yet in peer_info (Identify not received), the proof is
    /// buffered in `pending_verified_proofs` and applied when Identify completes.
    ///
    /// Returns Some(new_score) if the peer exists and needs a GossipSub score update,
    /// None if the peer is unknown/buffered or unchanged.
    pub(crate) fn record_verified_proof(
        &mut self,
        peer_id: &libp2p::PeerId,
        public_key: Vec<u8>,
    ) -> Option<f64> {
        let Some(peer_info) = self.peer_info.get_mut(peer_id) else {
            // Peer not in peer_info yet (Identify not received).
            // Buffer the proof to apply when Identify completes.
            self.pending_verified_proofs.insert(*peer_id, public_key);
            return None;
        };

        // Look up the validator by public key to get their address
        let validator_address = self
            .validator_set
            .iter()
            .find_map(|v| v.address_for_public_key(&public_key));

        let is_in_validator_set = validator_address.is_some();

        let new_type = peer_info
            .peer_type
            .with_validator_status(is_in_validator_set);

        // Clone old info for metrics before updating fields
        let old_peer_info = peer_info.clone();

        // Store the public key from the verified proof
        peer_info.consensus_public_key = Some(public_key.clone());

        // Set consensus_address only if in validator set (for display/metrics)
        peer_info.consensus_address = validator_address.map(|s| s.to_string());

        apply_peer_type_change(
            peer_id,
            peer_info,
            &old_peer_info,
            new_type,
            &mut self.metrics,
        )
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
            validator_set: HashSet::new(),
            metrics,
            local_node,
            peer_info: HashMap::new(),
            pending_verified_proofs: HashMap::new(),
        }
    }

    /// Determine the peer type based on peer ID and identify info
    ///
    /// Note: Validator status is determined via validator proof protocol,
    /// not from the Identify protocol. This only returns persistent peer status.
    pub(crate) fn peer_type(
        &self,
        peer_id: &libp2p::PeerId,
        connection_id: libp2p::swarm::ConnectionId,
    ) -> PeerType {
        let is_persistent = self.persistent_peer_ids.contains(peer_id)
            || self.is_persistent_peer_by_address(connection_id);

        // Validator status is now determined via validator proof protocol
        let is_validator = false;

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
            // Use GossipSub score if available, otherwise use internal score based on peer type
            let new_score = gossipsub.peer_score(peer_id).unwrap_or(peer_info.score);
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
        let peer_type = self.peer_type(&peer_id, connection_id);

        // Track persistent peers
        if peer_type.is_persistent() {
            self.persistent_peer_ids.insert(peer_id);
        }

        // Use actual connection address (dialed for outbound, source for inbound)
        // This is more reliable than self-reported listen_addrs from identify
        let address = self
            .discovery
            .connections
            .get(&connection_id)
            .map(|conn| conn.remote_addr.clone())
            .unwrap_or_else(|| {
                // Fallback to identify listen_addrs if connection info not available
                info.listen_addrs
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "/ip4/0.0.0.0/tcp/0".parse().expect("valid multiaddr"))
            });

        // Parse agent_version to extract moniker
        let agent_info = crate::utils::parse_agent_version(&info.agent_version);

        // Determine connection direction from discovery layer
        let connection_direction = if self.discovery.is_outbound_peer(&peer_id) {
            Some(ConnectionDirection::Outbound)
        } else if self.discovery.is_inbound_peer(&peer_id) {
            Some(ConnectionDirection::Inbound)
        } else {
            None // ephemeral connection
        };

        // If peer already exists (additional connection), just update Identify-provided fields.
        // Keep existing state since they never fully disconnected.
        if let Some(existing) = self.peer_info.get_mut(&peer_id) {
            let old_peer_info = existing.clone();
            existing.moniker = agent_info.moniker;
            // Prefer outbound (dialed) addresses over inbound
            if connection_direction == Some(ConnectionDirection::Outbound)
                || existing.connection_direction != Some(ConnectionDirection::Outbound)
            {
                existing.address = address;
                existing.connection_direction = connection_direction;
            }
            // Preserve: peer_type, consensus_public_key, consensus_address, score, topics

            self.metrics
                .update_peer_labels(&peer_id, &old_peer_info, existing);
            return crate::peer_scoring::get_peer_score(existing.peer_type);
        }

        // New peer - create entry
        let mut score = crate::peer_scoring::get_peer_score(peer_type);
        let peer_info = PeerInfo {
            address,
            consensus_public_key: None,
            consensus_address: None,
            moniker: agent_info.moniker,
            peer_type,
            connection_direction,
            score,
            topics: Default::default(),
        };

        // Record peer information in metrics (subject to 100 slot limit)
        self.metrics.record_new_peer(&peer_id, &peer_info);

        // Store in State
        self.peer_info.insert(peer_id, peer_info);

        // Check for pending verified proof (proof verification completed before Identify).
        // If found, apply it now that PeerInfo exists.
        if let Some(public_key) = self.pending_verified_proofs.remove(&peer_id) {
            if let Some(new_score) = self.record_verified_proof(&peer_id, public_key) {
                score = new_score;
            }
        }

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

    /// Update peer's persistent status, recalculate score, and update GossipSub
    fn update_peer_persistent_status(
        peer_id: libp2p::PeerId,
        peer_info: Option<&mut PeerInfo>,
        is_persistent: bool,
        swarm: &mut libp2p::Swarm<Behaviour>,
    ) {
        let Some(peer_info) = peer_info else {
            return;
        };

        peer_info.peer_type = peer_info.peer_type.with_persistent(is_persistent);

        // Recalculate score
        let new_score = crate::peer_scoring::get_peer_score(peer_info.peer_type);
        peer_info.score = new_score;

        // Update GossipSub score
        if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
            gossipsub.set_application_score(&peer_id, new_score);
        }

        tracing::debug!(
            %peer_id,
            %is_persistent,
            peer_type = ?peer_info.peer_type,
            "Updated peer persistent status"
        );
    }

    /// Add a persistent peer at runtime
    pub(crate) fn add_persistent_peer(
        &mut self,
        addr: Multiaddr,
        swarm: &mut libp2p::Swarm<Behaviour>,
    ) -> Result<(), PersistentPeerError> {
        // Check if already exists
        if self.persistent_peer_addrs.contains(&addr) {
            return Err(PersistentPeerError::AlreadyExists);
        }

        // Extract PeerId from multiaddr if present
        if let Some(peer_id) = extract_peer_id_from_multiaddr(&addr) {
            self.persistent_peer_ids.insert(peer_id);

            // Update peer type and score if already connected
            Self::update_peer_persistent_status(
                peer_id,
                self.peer_info.get_mut(&peer_id),
                true,
                swarm,
            );
        }

        // Add to persistent peer list
        self.persistent_peer_addrs.push(addr.clone());

        // Update discovery layer to add this as a bootstrap node
        self.discovery.add_bootstrap_node(addr.clone());

        // Attempt to dial the new persistent peer
        if let Err(e) = swarm.dial(addr.clone()) {
            tracing::warn!(
                error = %e,
                addr = %addr,
                "Failed to dial newly added persistent peer, will retry via discovery"
            );
            // Don't return error - the peer is added, dialing might succeed later
        }

        Ok(())
    }

    /// Remove a persistent peer at runtime
    pub(crate) fn remove_persistent_peer(
        &mut self,
        addr: Multiaddr,
        swarm: &mut libp2p::Swarm<Behaviour>,
    ) -> Result<(), PersistentPeerError> {
        // Check if exists and remove from persistent peer list
        let Some(pos) = self.persistent_peer_addrs.iter().position(|a| a == &addr) else {
            return Err(PersistentPeerError::NotFound);
        };

        self.persistent_peer_addrs.remove(pos);

        // Look up the peer_id from discovery, learned via TLS/noise handshake
        // when we successfully connected to this address
        let peer_id = self.discovery.get_peer_id_for_addr(&addr);

        if let Some(peer_id) = peer_id {
            self.persistent_peer_ids.remove(&peer_id);

            // Update peer type and score if connected
            Self::update_peer_persistent_status(
                peer_id,
                self.peer_info.get_mut(&peer_id),
                false,
                swarm,
            );

            // If peer is connected, disconnect it if
            // - `persistent_peers_only` is configured,
            // - or outbound connection exists
            // Do not disconnect if there are inbound connections as the peer might have us as their persistent peer
            let should_disconnect =
                self.local_node.persistent_peers_only || !self.discovery.is_inbound_peer(&peer_id);

            if swarm.is_connected(&peer_id) && should_disconnect {
                let _ = swarm.disconnect_peer_id(peer_id);
                tracing::info!(%peer_id, %addr, "Disconnecting from removed persistent peer");
            }
        }

        // Cancel any in-progress dial attempts for this address and peer
        self.discovery.cancel_dial_attempts(&addr, peer_id);

        // Update discovery layer
        self.discovery.remove_bootstrap_node(&addr);

        Ok(())
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

/// Helper to apply a peer type change, updating score and metrics.
///
/// Takes old_peer_info for stale metric labels (before any modifications)
/// and uses peer_info for current metric labels (after modifications).
/// Returns Some(new_score) if any label field changed, None otherwise.
fn apply_peer_type_change(
    peer_id: &libp2p::PeerId,
    peer_info: &mut PeerInfo,
    old_peer_info: &PeerInfo,
    new_type: PeerType,
    metrics: &mut NetworkMetrics,
) -> Option<f64> {
    // Update peer type and score
    let new_score = crate::peer_scoring::get_peer_score(new_type);
    peer_info.peer_type = new_type;
    peer_info.score = new_score;

    // Update metrics (marks old stale if labels changed)
    metrics
        .update_peer_labels(peer_id, old_peer_info, peer_info)
        .then_some(new_score)
}

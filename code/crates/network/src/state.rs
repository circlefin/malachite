//! Network state management

use std::collections::{HashMap, HashSet};

use libp2p::identify;
use libp2p::request_response::InboundRequestId;
use libp2p::Multiaddr;
use malachitebft_discovery as discovery;
use malachitebft_sync as sync;

use crate::behaviour::Behaviour;
use crate::metrics::Metrics as NetworkMetrics;
use crate::{Channel, ChannelNames, PeerType};
use malachitebft_discovery::ConnectionDirection;

/// Local node information
#[derive(Clone, Debug)]
pub struct LocalNodeInfo {
    pub moniker: String,
    pub peer_id: libp2p::PeerId,
    pub listen_addr: Multiaddr,
}

/// Peer information without slot number (for State, which has no cardinality limits)
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub address: Multiaddr,
    pub moniker: String,
    pub peer_type: PeerType,
    pub connection_direction: Option<ConnectionDirection>, // None if ephemeral (unknown)
    pub score: f64,
    pub topics: HashSet<String>, // Set of topics peer is in mesh for (e.g., "/consensus", "/liveness")
}

#[derive(Debug)]
pub struct State {
    pub sync_channels: HashMap<InboundRequestId, sync::ResponseChannel>,
    pub discovery: discovery::Discovery<Behaviour>,
    pub persistent_peer_ids: HashSet<libp2p::PeerId>,
    pub persistent_peer_addrs: Vec<Multiaddr>,
    pub(crate) metrics: NetworkMetrics,
    /// Local node information
    pub local_node: Option<LocalNodeInfo>,
    /// Detailed peer information indexed by PeerId (for RPC queries and metrics)
    pub peer_info: HashMap<libp2p::PeerId, PeerInfo>,
}

impl State {
    pub(crate) fn new(
        discovery: discovery::Discovery<Behaviour>,
        persistent_peer_addrs: Vec<Multiaddr>,
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
            metrics,
            local_node: None,
            peer_info: HashMap::new(),
        }
    }

    /// Determine the peer type based on peer ID and identify info
    pub(crate) fn peer_type(&self, peer_id: &libp2p::PeerId, info: &identify::Info) -> PeerType {
        let is_persistent =
            self.persistent_peer_ids.contains(peer_id) || self.is_persistent_peer_by_address(info);

        PeerType::from(is_persistent)
    }

    /// Check if a peer is a persistent peer by matching its addresses against persistent peer addresses
    fn is_persistent_peer_by_address(&self, info: &identify::Info) -> bool {
        // Check if any of the peer's listen addresses match a persistent peer address
        // We strip the /p2p/<peer_id> component for comparison
        for peer_addr in &info.listen_addrs {
            let peer_addr_without_p2p = strip_peer_id_from_multiaddr(peer_addr);

            for persistent_addr in &self.persistent_peer_addrs {
                let persistent_addr_without_p2p = strip_peer_id_from_multiaddr(persistent_addr);

                if peer_addr_without_p2p == persistent_addr_without_p2p {
                    return true;
                }
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

    /// Record peer information after Identify completes
    pub(crate) fn record_peer_info(&mut self, peer_id: libp2p::PeerId, info: &identify::Info) {
        // Determine peer type
        let peer_type = self.peer_type(&peer_id, info);

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

        // Extract moniker from agent_version (format: "moniker=app-0")
        let moniker = info
            .agent_version
            .strip_prefix("moniker=")
            .unwrap_or("unknown")
            .to_string();

        // Record peer information in State
        let peer_info = PeerInfo {
            address,
            moniker,
            peer_type,
            connection_direction,
            score: 0.0, // Initial score, will be updated by update_peer_info
            topics: Default::default(), // Empty set, will be updated by update_peer_info
        };

        // Record peer information in metrics (subject to 100 slot limit)
        self.metrics.record_peer_info(&peer_id, &peer_info);

        // Store in State
        self.peer_info.insert(peer_id, peer_info);
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

use std::collections::HashSet;

use malachitebft_metrics::prometheus::encoding::EncodeLabelSet;
use malachitebft_metrics::prometheus::metrics::family::Family;
use malachitebft_metrics::prometheus::metrics::gauge::Gauge;
use malachitebft_metrics::Registry;
use tracing::{debug, warn};

// Make prometheus_client available for the derive macro
use malachitebft_metrics::prometheus as prometheus_client;

use crate::state::{LocalNodeInfo, PeerInfo};
use crate::utils::Slots;
use crate::PeerType;
use libp2p::PeerId;

/// Maximum number of peer slots to track in metrics (to prevent unbounded memory growth)
const MAX_PEER_SLOTS: usize = 100;

/// Labels for peer info metrics
/// Note: score is the gauge VALUE
/// Note: mesh membership is tracked in separate peer_mesh_membership metric
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct PeerInfoLabels {
    slot: String,
    peer_moniker: String,
    peer_id: String,
    address: String,
    peer_type: PeerType,
    connection_direction: String, // "outbound" (we dialed), "inbound" (they dialed), or "unknown" (ephemeral)
}

/// Labels for per-topic mesh membership metric
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct MeshMembershipLabels {
    peer_id: String,
    peer_moniker: String,
    topic: String, // "/consensus", "/liveness", "/proposal_parts"
}

impl PeerInfo {
    /// Convert to Prometheus metric labels (with slot number)
    pub(crate) fn to_labels(&self, peer_id: &PeerId, slot: usize) -> PeerInfoLabels {
        PeerInfoLabels {
            slot: slot.to_string(),
            peer_moniker: self.moniker.clone(),
            peer_id: peer_id.to_string(),
            address: self.address.to_string(),
            peer_type: self.peer_type,
            connection_direction: self
                .connection_direction
                .map(|d| d.as_str())
                .unwrap_or("unknown")
                .to_string(),
        }
    }
}

/// Labels for local node info (peer_id and listen address)
/// Note: moniker is automatically added by SharedRegistry.with_prefix()
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct LocalNodeLabels {
    peer_id: String,
    listen_addr: String,
}

/// Network metrics
pub(crate) struct Metrics {
    /// Info about the local node (moniker, peer_id, listen address)
    local_node_info: Family<LocalNodeLabels, Gauge>,
    /// Discovered peers with basic info (gauge value = peer score)
    discovered_peers: Family<PeerInfoLabels, Gauge>,
    /// Per-peer, per-topic mesh membership (1 = in mesh, 0 = not in mesh)
    peer_mesh_membership: Family<MeshMembershipLabels, Gauge>,
    /// PeerId to slot number mapping
    peer_slots: Slots<PeerId>,
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metrics")
            .field("assigned_slots_count", &self.peer_slots.assigned())
            .field("available_slots_count", &self.peer_slots.available())
            .finish()
    }
}

impl Metrics {
    pub(crate) fn new(registry: &mut Registry) -> Self {
        let local_node_info = Family::<LocalNodeLabels, Gauge>::default();
        let peer_info = Family::<PeerInfoLabels, Gauge>::default();
        let mesh_membership = Family::<MeshMembershipLabels, Gauge>::default();

        registry.register(
            "local_node_info",
            "Information about the local node (moniker, peer_id, listen_addr)",
            local_node_info.clone(),
        );

        registry.register(
            "discovered_peers",
            "Discovered/connected peers with basic info (gauge value = peer score)",
            peer_info.clone(),
        );

        registry.register(
            "peer_mesh_membership",
            "Per-peer, per-topic gossipsub mesh membership (1 = in mesh, 0 = not in mesh)",
            mesh_membership.clone(),
        );

        Self {
            local_node_info,
            discovered_peers: peer_info,
            peer_mesh_membership: mesh_membership,
            peer_slots: Slots::new(MAX_PEER_SLOTS),
        }
    }

    /// Set the local node information (called once at startup)
    pub(crate) fn set_local_node_info(&self, info: &LocalNodeInfo) {
        let labels = LocalNodeLabels {
            peer_id: info.peer_id.to_string(),
            listen_addr: info.listen_addr.to_string(),
        };
        self.local_node_info.get_or_create(&labels).set(1);
    }

    /// Update a peer's score and mesh membership metrics
    pub(crate) fn update_peer_metrics(
        &mut self,
        peer_id: &PeerId,
        peer_info: &PeerInfo,
        score: f64,
        new_topics: Option<HashSet<String>>,
    ) -> Result<(), ()> {
        // Get slot from peer_to_slot
        let slot = match self.peer_slots.assign(*peer_id) {
            Some(slot) => slot,
            None => return Err(()), // Peer not tracked in metrics
        };

        // Update topics if provided
        if let Some(ref new_topics) = new_topics {
            // Update mesh membership metrics for topics that changed
            let old_topics = &peer_info.topics;

            // Topics that were removed: set to 0
            for topic in old_topics.difference(new_topics) {
                let mesh_labels = MeshMembershipLabels {
                    peer_id: peer_id.to_string(),
                    peer_moniker: peer_info.moniker.clone(),
                    topic: topic.clone(),
                };
                self.peer_mesh_membership.get_or_create(&mesh_labels).set(0);
            }

            // Topics that were added: set to 1
            for topic in new_topics.difference(old_topics) {
                let mesh_labels = MeshMembershipLabels {
                    peer_id: peer_id.to_string(),
                    peer_moniker: peer_info.moniker.clone(),
                    topic: topic.clone(),
                };
                self.peer_mesh_membership.get_or_create(&mesh_labels).set(1);
            }
        }

        // Update peer score in discovered_peers metric
        let labels = peer_info.to_labels(peer_id, slot);
        self.discovered_peers
            .get_or_create(&labels)
            .set(score as i64);

        Ok(())
    }

    /// Free a slot when a peer disconnects
    /// Note: Caller should also remove peer from State.peer_info
    pub(crate) fn free_slot(&mut self, peer_id: &PeerId, peer_info: &PeerInfo) {
        // Return slot to available pool
        if let Some(slot) = self.peer_slots.release(peer_id) {
            // Set discovered_peers to i64::MIN to signal disconnection
            // This allows distinguishing stale entries from active peers in metrics
            let labels = peer_info.to_labels(peer_id, slot);
            self.discovered_peers.get_or_create(&labels).set(i64::MIN);

            // Clear mesh membership metrics - peer is no longer in any mesh
            for topic in &peer_info.topics {
                let mesh_labels = MeshMembershipLabels {
                    peer_id: peer_id.to_string(),
                    peer_moniker: peer_info.moniker.clone(),
                    topic: topic.clone(),
                };
                self.peer_mesh_membership.get_or_create(&mesh_labels).set(0);
            }

            debug!("Freed slot {slot} for peer {peer_id}");
        }
    }

    pub(crate) fn record_peer_info(&mut self, peer_id: &PeerId, peer_info: &PeerInfo) {
        // Check if peer already has a slot (re-identification case)
        if self.peer_slots.contains(peer_id) {
            // Peer already tracked in metrics, labels are already set
            // Score will be updated by update_peer_info
            return;
        }

        // Try to assign a new slot (subject to 100 slot limit)
        let Some(slot) = self.peer_slots.assign(*peer_id) else {
            // Failed to assign slot (all slots full)
            warn!("No available metric slots for peer {peer_id}");
            return;
        };

        // Create labels for initial metrics (score will be updated by update_peer_info)
        let labels = peer_info.to_labels(peer_id, slot);
        self.discovered_peers.get_or_create(&labels).set(0);
    }
}

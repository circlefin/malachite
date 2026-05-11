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
    peer_type: PeerType,
    consensus_address: String, // Consensus address for validators, "none" for non-validators
}

/// Labels for per-topic mesh membership metric
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct MeshMembershipLabels {
    peer_id: String,
    peer_moniker: String,
    topic: String, // "/consensus", "/liveness", "/proposal_parts"
}

/// Labels for explicit peer metric
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct ExplicitPeerLabels {
    peer_id: String,
    peer_moniker: String,
}

impl PeerInfo {
    /// Convert to Prometheus metric labels (with slot number)
    pub(crate) fn to_labels(&self, peer_id: &PeerId, slot: usize) -> PeerInfoLabels {
        PeerInfoLabels {
            slot: slot.to_string(),
            peer_moniker: self.moniker.clone(),
            peer_id: peer_id.to_string(),
            peer_type: self.peer_type,
            // Show verified consensus_address if known, "none" if never verified
            consensus_address: self
                .consensus_address
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        }
    }

    /// Check if label-relevant fields match another PeerInfo.
    /// Used to determine if metrics need to be updated (stale marking).
    pub(crate) fn labels_match(&self, other: &PeerInfo) -> bool {
        self.moniker == other.moniker
            && self.peer_type == other.peer_type
            && self.consensus_address == other.consensus_address
    }
}

/// Labels for local node info (peer_id and listen address)
/// Note: moniker is automatically added by SharedRegistry.with_prefix()
/// Note: gauge value = is_validator (1 = validator, 0 = not validator)
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct LocalNodeLabels {
    peer_id: String,
    listen_addr: String,
    consensus_address: String, // Consensus address if validator, "none" otherwise
}

/// Network metrics
pub(crate) struct Metrics {
    /// Info about the local node (moniker, peer_id, listen address)
    local_node_info: Family<LocalNodeLabels, Gauge>,
    /// Discovered peers with basic info (gauge value = peer score)
    discovered_peers: Family<PeerInfoLabels, Gauge>,
    /// Per-peer, per-topic mesh membership.
    /// Entry present with value 1 = in mesh; entry absent = not in mesh.
    peer_mesh_membership: Family<MeshMembershipLabels, Gauge>,
    /// Explicit peers in gossipsub.
    /// Entry present with value 1 = currently configured as an explicit peer;
    /// entry absent = not explicit / disconnected.
    explicit_peers: Family<ExplicitPeerLabels, Gauge>,
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
        let explicit_peers = Family::<ExplicitPeerLabels, Gauge>::default();

        registry.register(
            "local_node_info",
            "Information about the local node (gauge value: 1 = validator, 0 = not validator)",
            local_node_info.clone(),
        );

        registry.register(
            "discovered_peers",
            "Discovered/connected peers with basic info (gauge value = peer score)",
            peer_info.clone(),
        );

        registry.register(
            "peer_mesh_membership",
            "Per-peer, per-topic gossipsub mesh membership (entry present with value 1 = in mesh; entry absent = not in mesh)",
            mesh_membership.clone(),
        );

        registry.register(
            "explicit_peers",
            "Peers added as explicit peers in gossipsub (entry present with value 1 = active; entry absent = not explicit / disconnected)",
            explicit_peers.clone(),
        );

        Self {
            local_node_info,
            discovered_peers: peer_info,
            peer_mesh_membership: mesh_membership,
            explicit_peers,
            peer_slots: Slots::new(MAX_PEER_SLOTS),
        }
    }

    /// Set the local node information (called once at startup and updated when validator set changes)
    /// Gauge value: 1 if validator, 0 if not
    pub(crate) fn set_local_node_info(&self, info: &LocalNodeInfo) {
        // The consensus_address label always shows the configured address (or "none" if not configured).
        // The gauge VALUE indicates current validator status (1 = active validator, 0 = not).
        let labels = LocalNodeLabels {
            peer_id: info.peer_id.to_string(),
            listen_addr: info.listen_addr.to_string(),
            consensus_address: info
                .consensus_address
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        };
        // Set gauge to 1 if validator, 0 if not
        let gauge_value = if info.is_validator { 1 } else { 0 };
        self.local_node_info.get_or_create(&labels).set(gauge_value);
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

            // Topics that were removed
            for topic in old_topics.difference(new_topics) {
                let mesh_labels = MeshMembershipLabels {
                    peer_id: peer_id.to_string(),
                    peer_moniker: peer_info.moniker.clone(),
                    topic: topic.clone(),
                };
                self.peer_mesh_membership.remove(&mesh_labels);
            }

            // Topics that were added
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
            // Remove the peer's entry from the discovered_peers family entirely.
            let labels = peer_info.to_labels(peer_id, slot);
            self.discovered_peers.remove(&labels);

            // Clear mesh membership metrics
            for topic in &peer_info.topics {
                let mesh_labels = MeshMembershipLabels {
                    peer_id: peer_id.to_string(),
                    peer_moniker: peer_info.moniker.clone(),
                    topic: topic.clone(),
                };
                self.peer_mesh_membership.remove(&mesh_labels);
            }

            debug!("Freed slot {slot} for peer {peer_id}");
        }
    }

    /// Record a peer as an explicit peer in gossipsub
    pub(crate) fn record_explicit_peer(&self, peer_id: &PeerId, moniker: &str) {
        let labels = ExplicitPeerLabels {
            peer_id: peer_id.to_string(),
            peer_moniker: moniker.to_string(),
        };
        self.explicit_peers.get_or_create(&labels).set(1);
    }

    /// Remove an explicit peer entry (peer is disconnected or no longer explicit).
    pub(crate) fn mark_explicit_peer_stale(&self, peer_id: &PeerId, moniker: &str) {
        let labels = ExplicitPeerLabels {
            peer_id: peer_id.to_string(),
            peer_moniker: moniker.to_string(),
        };
        self.explicit_peers.remove(&labels);
    }

    /// Record metrics for a new peer (assigns slot if needed).
    pub(crate) fn record_new_peer(&mut self, peer_id: &PeerId, peer_info: &PeerInfo) {
        let slot = if let Some(existing_slot) = self.peer_slots.get(peer_id) {
            existing_slot
        } else {
            let Some(new_slot) = self.peer_slots.assign(*peer_id) else {
                warn!("No available metric slots for peer {peer_id}");
                return;
            };
            new_slot
        };

        let labels = peer_info.to_labels(peer_id, slot);
        self.discovered_peers
            .get_or_create(&labels)
            .set(peer_info.score as i64);
    }

    /// Update metrics for an existing peer when labels may have changed.
    ///
    /// Compares old and new peer info:
    /// - If labels changed: removes the old entry from the metric family and
    ///   creates a new entry under the updated labels.
    /// - If labels unchanged: just updates the score.
    ///
    /// Returns true if labels changed.
    pub(crate) fn update_peer_labels(
        &mut self,
        peer_id: &PeerId,
        old_peer_info: &PeerInfo,
        new_peer_info: &PeerInfo,
    ) -> bool {
        let Some(slot) = self.peer_slots.get(peer_id) else {
            return false;
        };

        let labels_changed = !old_peer_info.labels_match(new_peer_info);

        if labels_changed {
            // Remove the old entry so it does not leak as a permanent stale
            // time series.
            let old_labels = old_peer_info.to_labels(peer_id, slot);
            tracing::debug!(%peer_id, ?old_labels, "Removing stale peer metric entry");
            self.discovered_peers.remove(&old_labels);
        }

        // Create/update metric entry with current labels
        let new_labels = new_peer_info.to_labels(peer_id, slot);
        self.discovered_peers
            .get_or_create(&new_labels)
            .set(new_peer_info.score as i64);

        labels_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_scoring::FULL_NODE_SCORE;
    use crate::state::PeerInfo;
    use libp2p::Multiaddr;
    use malachitebft_metrics::prometheus::encoding::text::encode;
    use malachitebft_metrics::Registry;
    use std::collections::HashSet;

    /// Build a `PeerInfo` for testing. The `address` is included to exercise
    /// the historical bug path: in the broken implementation, a new address on
    /// the same peer produced a new time series because the address was part
    /// of the label set.
    fn peer_info(moniker: &str, address: &str) -> PeerInfo {
        PeerInfo {
            moniker: moniker.to_string(),
            address: address.parse::<Multiaddr>().unwrap(),
            consensus_address: None,
            consensus_public_key: None,
            peer_type: PeerType::new(false, false),
            connection_direction: None,
            score: FULL_NODE_SCORE,
            topics: HashSet::new(),
            is_explicit: false,
        }
    }

    /// Count time series for a metric name in the encoded Prometheus output.
    /// Every `<metric_name>{...}` line is a unique series.
    fn metric_series_count(registry: &Registry, metric_name: &str) -> usize {
        let mut buf = String::new();
        encode(&mut buf, registry).unwrap();
        let prefix = format!("{metric_name}{{");
        buf.lines().filter(|line| line.starts_with(&prefix)).count()
    }

    fn discovered_peers_series_count(registry: &Registry) -> usize {
        metric_series_count(registry, "discovered_peers")
    }

    /// Repeatedly connect and disconnect the same peer using a fresh ephemeral
    /// port each time.
    #[test]
    fn discovered_peers_bounded_under_ephemeral_port_churn() {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(&mut registry);
        let peer_id = libp2p::PeerId::random();

        // 500 churns is more than enough to expose unbounded growth — the
        // production bug saw hundreds of thousands of series per peer.
        for port in 0..500u16 {
            let addr = format!("/ip4/10.0.0.1/tcp/{port}");
            let info = peer_info("peer-a", &addr);
            metrics.record_new_peer(&peer_id, &info);
            metrics.free_slot(&peer_id, &info);
        }

        // After all peers have disconnected, no series should remain.
        assert_eq!(
            discovered_peers_series_count(&registry),
            0,
            "disconnect must prune the discovered_peers entry"
        );
    }

    /// While connected, the same peer must produce at most one series even
    /// across many reconnections from different ephemeral addresses.
    #[test]
    fn discovered_peers_single_series_per_connected_peer() {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(&mut registry);
        let peer_id = libp2p::PeerId::random();

        for port in 0..100u16 {
            let addr = format!("/ip4/10.0.0.1/tcp/{port}");
            let info = peer_info("peer-a", &addr);
            // Simulate a label-change update with a fresh address on each
            // iteration; the previous PeerInfo address is irrelevant for
            // labels and should not leak.
            let previous = peer_info(
                "peer-a",
                &format!("/ip4/10.0.0.1/tcp/{}", port.wrapping_sub(1)),
            );
            metrics.record_new_peer(&peer_id, &info);
            metrics.update_peer_labels(&peer_id, &previous, &info);
        }

        let count = discovered_peers_series_count(&registry);
        assert_eq!(
            count, 1,
            "same peer reconnecting from different ephemeral ports must yield one series, got {count}"
        );
    }

    /// 100 distinct peers connect, then all disconnect. Cardinality should
    /// peak at the connected count and return to zero.
    #[test]
    fn discovered_peers_cardinality_tracks_connected_peers() {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(&mut registry);

        let peers: Vec<_> = (0..100u16)
            .map(|i| {
                let id = libp2p::PeerId::random();
                let info = peer_info(&format!("peer-{i}"), &format!("/ip4/10.0.0.{i}/tcp/26656"));
                (id, info)
            })
            .collect();

        for (id, info) in &peers {
            metrics.record_new_peer(id, info);
        }
        assert_eq!(
            discovered_peers_series_count(&registry),
            peers.len(),
            "one series per connected peer while connected"
        );

        for (id, info) in &peers {
            metrics.free_slot(id, info);
        }
        assert_eq!(
            discovered_peers_series_count(&registry),
            0,
            "all series must be pruned once every peer disconnects"
        );
    }

    #[test]
    fn peer_mesh_membership_pruned_on_leave() {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(&mut registry);
        let peer_id = libp2p::PeerId::random();

        let mut info = peer_info("peer-a", "/ip4/10.0.0.1/tcp/26656");
        metrics.record_new_peer(&peer_id, &info);

        let consensus: HashSet<String> = ["/consensus".to_string()].into_iter().collect();
        let none: HashSet<String> = HashSet::new();

        for _ in 0..200 {
            // Join /consensus
            metrics
                .update_peer_metrics(&peer_id, &info, info.score, Some(consensus.clone()))
                .unwrap();
            info.topics = consensus.clone();

            // Leave /consensus — entry must be removed, not zeroed
            metrics
                .update_peer_metrics(&peer_id, &info, info.score, Some(none.clone()))
                .unwrap();
            info.topics = none.clone();
        }

        assert_eq!(
            metric_series_count(&registry, "peer_mesh_membership"),
            0,
            "leaving a topic mesh must prune the peer_mesh_membership entry"
        );
    }

    #[test]
    fn peer_mesh_membership_pruned_on_disconnect() {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(&mut registry);
        let peer_id = libp2p::PeerId::random();

        let topics: HashSet<String> = ["/consensus", "/liveness", "/proposal_parts"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut info = peer_info("peer-a", "/ip4/10.0.0.1/tcp/26656");
        metrics.record_new_peer(&peer_id, &info);
        metrics
            .update_peer_metrics(&peer_id, &info, info.score, Some(topics.clone()))
            .unwrap();
        info.topics = topics;

        assert_eq!(metric_series_count(&registry, "peer_mesh_membership"), 3);

        metrics.free_slot(&peer_id, &info);

        assert_eq!(
            metric_series_count(&registry, "peer_mesh_membership"),
            0,
            "disconnect must prune all mesh-membership entries for the peer"
        );
    }

    #[test]
    fn explicit_peers_pruned_on_stale() {
        let mut registry = Registry::default();
        let metrics = Metrics::new(&mut registry);

        for i in 0..200 {
            let peer_id = libp2p::PeerId::random();
            let moniker = format!("peer-{i}");
            metrics.record_explicit_peer(&peer_id, &moniker);
            metrics.mark_explicit_peer_stale(&peer_id, &moniker);
        }

        assert_eq!(
            metric_series_count(&registry, "explicit_peers"),
            0,
            "marking an explicit peer stale must prune the entry"
        );
    }
}

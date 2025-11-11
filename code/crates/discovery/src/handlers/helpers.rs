use tracing::info;

use crate::{ConnectionDirection, Discovery, DiscoveryClient};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn total_active_connections_len(&self) -> usize {
        self.active_connections.values().map(Vec::len).sum()
    }

    fn outbound_connections_len(&self) -> usize {
        // Count connections by actual socket direction (we dialed them)
        self.active_connections
            .values()
            .flatten()
            .filter(|connection_id| {
                matches!(
                    self.connection_directions.get(connection_id),
                    Some(ConnectionDirection::Outbound)
                )
            })
            .count()
    }

    fn inbound_connections_len(&self) -> usize {
        // Count connections by actual socket direction (they dialed us)
        self.active_connections
            .values()
            .flatten()
            .filter(|connection_id| {
                matches!(
                    self.connection_directions.get(connection_id),
                    Some(ConnectionDirection::Inbound)
                )
            })
            .count()
    }

    pub(crate) fn update_discovery_metrics(&mut self) {
        let num_active_peers = self.active_connections.len();
        let num_active_connections = self.total_active_connections_len();
        let num_outbound_peers = self.outbound_peers.len();
        let num_outbound_connections = self.outbound_connections_len();

        // Count peers that are ONLY inbound (not in both sets)
        // This fixes the double-counting issue when peers dial each other simultaneously
        let num_only_inbound = self
            .inbound_peers
            .iter()
            .filter(|peer| !self.outbound_peers.contains_key(peer))
            .count();

        let num_inbound_connections = self.inbound_connections_len();

        // Ephemeral = peers in neither set
        let num_ephemeral_peers =
            num_active_peers.saturating_sub(num_outbound_peers + num_only_inbound);
        let num_ephemeral_connections = num_active_connections
            .saturating_sub(num_outbound_connections + num_inbound_connections);

        if !self.is_enabled() {
            info!(
                "Active connections: {} (peers: {}), Outbound connections: {} (peers: {}), Inbound connections: {} (peers: {})",
                num_active_connections,
                num_active_peers,
                num_outbound_connections,
                num_outbound_peers,
                num_inbound_connections,
                num_only_inbound
            );
        } else {
            info!(
                "Active connections: {} (peers: {}), Outbound connections: {} (peers: {}), Inbound connections: {} (peers: {}), Ephemeral connections: {} (peers: {})",
                num_active_connections,
                num_active_peers,
                num_outbound_connections,
                num_outbound_peers,
                num_inbound_connections,
                num_only_inbound,
                num_ephemeral_connections,
                num_ephemeral_peers
            );
        }

        self.metrics.set_connections_status(
            num_active_peers,
            num_active_connections,
            num_outbound_peers,
            num_outbound_connections,
            num_only_inbound,
            num_inbound_connections,
            num_ephemeral_peers,
            num_ephemeral_connections,
        );
    }
}

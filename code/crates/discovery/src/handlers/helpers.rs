use tracing::info;

use crate::{Discovery, DiscoveryClient};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn total_active_connections_len(&self) -> usize {
        self.active_connections.values().map(Vec::len).sum()
    }

    fn outbound_connections_len(&self) -> usize {
        self.active_connections
            .iter()
            .filter_map(|(peer_id, connection_ids)| {
                if self.outbound_peers.contains_key(peer_id) {
                    Some(connection_ids.len())
                } else {
                    None
                }
            })
            .sum()
    }

    fn inbound_connections_len(&self) -> usize {
        self.active_connections
            .iter()
            .filter_map(|(peer_id, connection_ids)| {
                if self.inbound_peers.contains(peer_id) {
                    Some(connection_ids.len())
                } else {
                    None
                }
            })
            .sum()
    }

    pub(crate) fn update_discovery_metrics(&mut self) {
        let num_active_peers = self.active_connections.len();
        let num_active_connections = self.total_active_connections_len();
        let num_outbound_peers = self.outbound_peers.len();
        let num_outbound_connections = self.outbound_connections_len();
        let num_inbound_peers = self.inbound_peers.len();
        let num_inbound_connections = self.inbound_connections_len();
        let num_ephemeral_peers = self
            .active_connections
            .len()
            .saturating_sub(num_outbound_peers + num_inbound_peers);
        let num_ephemeral_connections = num_active_connections
            .saturating_sub(num_outbound_connections + num_inbound_connections);

        if !self.is_enabled() {
            info!("Connections: {}", num_inbound_connections);
        } else {
            info!(
                "Active connections: {} (peers: {}), Outbound connections: {} (peers: {}), Inbound connections: {} (peers: {}), Ephemeral connections: {} (peers: {})",
                num_active_connections,
                num_active_peers,
                num_outbound_connections,
                num_outbound_peers,
                num_inbound_connections,
                num_inbound_peers,
                num_ephemeral_connections,
                num_ephemeral_peers,
            );
        }

        self.metrics.set_connections_status(
            num_active_peers,
            num_active_connections,
            num_outbound_peers,
            num_outbound_connections,
            num_inbound_peers,
            num_inbound_connections,
            num_ephemeral_peers,
            num_ephemeral_connections,
        );
    }
}

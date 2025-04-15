use tracing::info;

use crate::{Discovery, DiscoveryClient};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn active_connections_num_duplicates(&self) -> usize {
        self.active_connections
            .values()
            .map(|ids| ids.len() - 1)
            .sum()
    }

    pub(crate) fn update_connections_metrics(&mut self) {
        let num_inbound_connections = self.inbound_connections.len();
        let num_active_connections = self.active_connections_len();
        let num_outbound_connections = self.outbound_connections.len();
        let num_ephemeral_connections = num_active_connections
            .saturating_sub(num_outbound_connections + num_inbound_connections);

        if !self.is_enabled() {
            info!("Connections: {}", num_inbound_connections);
        } else {
            info!(
                "Active connections: {} (duplicates: {}), Outbound connections: {}, Inbound connections: {}, Ephemeral connections: {}",
                num_active_connections,
                self.active_connections_num_duplicates(),
                num_outbound_connections,
                num_inbound_connections,
                num_ephemeral_connections,
            );
        }

        self.metrics.set_connections_status(
            num_active_connections,
            num_outbound_connections,
            num_inbound_connections,
            num_ephemeral_connections,
        );
    }
}

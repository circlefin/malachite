use libp2p::{PeerId, Swarm};
use tracing::info;

use crate::{Discovery, DiscoveryClient};

impl Discovery {
    fn active_connections_num_duplicates(&self) -> usize {
        self.active_connections
            .values()
            .map(|ids| ids.len() - 1)
            .sum()
    }

    pub(crate) fn update_connections_metrics(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        let num_active_connections = self.active_connections_len();
        let num_outbound_connections = self.outbound_connections.len();
        let num_inbound_connections = self.inbound_connections.len();
        let num_ephemeral_connections = num_active_connections
            .saturating_sub(num_outbound_connections + num_inbound_connections);

        info!(
            "Active connections: {} (duplicates: {}), Outbound connections: {}, Inbound connections: {}, Ephemeral connections: {}",
            num_active_connections,
            self.active_connections_num_duplicates(),
            num_outbound_connections,
            num_inbound_connections,
            num_ephemeral_connections,
        );

        self.metrics.set_connections_status(
            num_active_connections,
            num_outbound_connections,
            num_inbound_connections,
            num_ephemeral_connections,
        );

        self.print_stats(swarm);
    }

    // This is purely for benchmarking purposes
    fn print_stats(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() {
            return;
        }

        let kbuckets: Vec<(u32, Vec<PeerId>)> = self.get_routing_table(swarm);

        let mut json = serde_json::json!({
            "timeBootstrap": self.metrics.initial_bootstrap_duration().as_millis(),
            "time": self.metrics.initial_discovery_duration().as_millis(),
            "localPeerId": swarm.local_peer_id(),
            "totalPeers": kbuckets.iter().map(|(_, peers)| peers.len()).sum::<usize>(),
            "subset": self.outbound_connections.keys().cloned().collect::<Vec<_>>(),
            "rejections": self.metrics.get_total_rejected_connect_requests(),
            "kbuckets": {}
        });

        for (index, peers) in kbuckets {
            json["kbuckets"][index.to_string()] = serde_json::to_value(peers).unwrap();
        }

        println!("{}", serde_json::to_string(&json).unwrap());
    }
}

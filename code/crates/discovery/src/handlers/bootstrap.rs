use libp2p::swarm;
use tracing::info;

use crate::{Discovery, DiscoveryClient, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub(crate) fn handle_successful_bootstrap(&mut self, swarm: &mut swarm::Swarm<C>) {
        // NOTE: A new bootstrap query is initiated every time a new peer is added
        // to the routing table (delayed with kad crate configuration parameter `automatic_bootstrap_throttle`
        // to avoid multiple queries in a short period of time).
        // Here, we only consider the first bootstrap query to determine the status of the discovery process.
        if self.state == State::Bootstrapping {
            info!(
                "Discovery bootstrap done in {}ms, found {} peers",
                self.metrics.elapsed().as_millis(),
                self.active_connections.len()
            );

            self.metrics.initial_bootstrap_finished();

            if self.active_connections.len() < self.config.num_outbound_peers {
                info!(
                    "Not enough active connections (got {}, expected {}) to select outbound peers",
                    self.active_connections.len(),
                    self.config.num_outbound_peers
                );

                self.initiate_extension_with_target(swarm, self.config.num_outbound_peers);
            } else {
                info!(
                    "Discovery found {} peers (expected {}) in {}ms",
                    self.discovered_peers.len(),
                    self.config.num_outbound_peers,
                    self.metrics.elapsed().as_millis()
                );

                self.adjust_peers(swarm);

                self.state = State::Idle;
            }
        }
    }

    pub(crate) fn handle_failed_bootstrap(&mut self) {
        if self.state == State::Bootstrapping {
            self.state = State::Idle;
        }
    }
}

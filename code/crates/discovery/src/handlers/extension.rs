use libp2p::{PeerId, Swarm};
use tracing::{debug, info, warn};

use crate::{request::RequestData, Discovery, DiscoveryClient, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn get_next_peer_to_peers_request(&self) -> Option<PeerId> {
        self.discovered_peers
            .iter()
            .find(|(peer_id, _)| !self.controller.peers_request.is_done_on(peer_id))
            .map(|(peer_id, _)| *peer_id)
    }

    pub(crate) fn initiate_extension_with_target(&mut self, swarm: &mut Swarm<C>, target: usize) {
        if let State::Extending(curr_target) = self.state {
            debug!(
                "Updating extension target from {} to {}",
                curr_target,
                curr_target + target
            );
            self.state = State::Extending(curr_target + target);

            return;
        }

        debug!(
            "Initiating discovery extension with a target of {} peers",
            target
        );
        self.state = State::Extending(target);
        self.make_extension_step(swarm); // trigger extension
    }

    pub(crate) fn make_extension_step(&mut self, swarm: &mut Swarm<C>) {
        if !self.is_enabled() {
            return;
        }

        let target = match self.state {
            State::Extending(target) => target,
            _ => {
                // Not in extending state
                return;
            }
        };

        let (is_idle, pending_connections_len, pending_peers_requests_len) =
            self.controller.is_idle();
        let rx_dial_len = self.controller.dial.queue_len();
        let rx_peers_request_len = self.controller.peers_request.queue_len();

        if is_idle && rx_dial_len == 0 && rx_peers_request_len == 0 {
            // Done when we found enough peers to which we did not request persistent connection yet
            // to potentially upgrade them to the outbound peers we are missing.
            if self
                .active_connections
                .iter()
                .filter(|(peer_id, _)| !self.controller.connect_request.is_done_on(peer_id))
                .count()
                < target
            {
                if let Some(peer_id) = self.get_next_peer_to_peers_request() {
                    debug!(
                        "Discovery extension in progress ({}ms), requesting peers from peer {}",
                        self.metrics.elapsed().as_millis(),
                        peer_id
                    );

                    self.controller
                        .peers_request
                        .add_to_queue(RequestData::new(peer_id), None);

                    return;
                } else {
                    warn!("No more peers to request peers from");
                }
            }

            info!("Discovery extension done");
            info!(
                "Discovery found {} peers (expected {}) in {}ms",
                self.discovered_peers.len(),
                self.config.num_outbound_peers,
                self.metrics.elapsed().as_millis()
            );

            self.adjust_peers(swarm);

            self.state = State::Idle;
        } else {
            debug!("Discovery extension in progress ({}ms), {} pending connections ({} in queue), {} pending requests ({} in queue)",
                self.metrics.elapsed().as_millis(),
                pending_connections_len,
                rx_dial_len,
                pending_peers_requests_len,
                rx_peers_request_len,
            );
        }
    }
}

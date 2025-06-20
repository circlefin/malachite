use libp2p::{
    request_response::{OutboundRequestId, ResponseChannel},
    PeerId, Swarm,
};
use tracing::{debug, error, trace};

use crate::{
    behaviour::{self, Response},
    request::RequestData,
    Discovery, DiscoveryClient, OutboundState,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_connect_request(&self) -> bool {
        self.controller.peers_request.can_perform()
    }

    fn should_connect_request(&self, request_data: &RequestData) -> bool {
        // Has not already requested, or has requested but retries are allowed
        !self
            .controller
            .connect_request
            .is_done_on(&request_data.peer_id())
            || request_data.retry.count() != 0
    }

    pub fn connect_request_peer(&mut self, swarm: &mut Swarm<C>, request_data: RequestData) {
        if !self.should_connect_request(&request_data) {
            return;
        }

        self.controller
            .connect_request
            .register_done_on(request_data.peer_id());

        // Do not count retries as new interactions
        if request_data.retry.count() == 0 {
            self.metrics.increment_total_connect_requests();
        }

        debug!(
            "Requesting persistent connection to peer {}, retry #{}",
            request_data.peer_id(),
            request_data.retry.count()
        );

        let request_id = swarm
            .behaviour_mut()
            .send_request(&request_data.peer_id(), behaviour::Request::Connect());

        self.controller
            .connect_request
            .register_in_progress(request_id, request_data);
    }

    pub(crate) fn handle_connect_request(
        &mut self,
        swarm: &mut Swarm<C>,
        channel: ResponseChannel<Response>,
        peer: PeerId,
    ) {
        let mut accepted: bool = false;

        if self.outbound_peers.contains_key(&peer) {
            debug!("Peer {peer} is already an outbound peer");

            accepted = true;
        } else if self.inbound_peers.contains(&peer) {
            debug!("Peer {peer} is already an inbound peer");

            accepted = true;
        } else if self.inbound_peers.len() < self.config.num_inbound_peers {
            debug!("Upgrading peer {peer} to inbound peer");

            self.inbound_peers.insert(peer);
            accepted = true;
        } else {
            debug!("Rejecting upgrade of peer {peer} to inbound peer as the limit is reached");
        }

        self.update_discovery_metrics();

        if swarm
            .behaviour_mut()
            .send_response(channel, behaviour::Response::Connect(accepted))
            .is_err()
        {
            error!("Error sending connect response to {peer}");
        } else {
            trace!("Sent connect response to {peer}");
        }
    }

    pub(crate) fn handle_connect_response(
        &mut self,
        swarm: &mut Swarm<C>,
        request_id: OutboundRequestId,
        peer: PeerId,
        accepted: bool,
    ) {
        self.controller
            .connect_request
            .remove_in_progress(&request_id);

        if accepted {
            debug!("Successfully upgraded peer {peer} to outbound peer");

            if let Some(state) = self.outbound_peers.get_mut(&peer) {
                *state = OutboundState::Confirmed;
            }

            // if all outbound peers are persistent, discovery is done
            if self
                .outbound_peers
                .values()
                .all(|state| *state == OutboundState::Confirmed)
            {
                debug!("All outbound peers are persistent");

                self.metrics.initial_discovery_finished();
                self.update_discovery_metrics();
            }
        } else {
            debug!("Peer {peer} rejected connection upgrade to outbound peer");

            self.metrics.increment_total_rejected_connect_requests();

            self.handle_connect_rejection(swarm, peer);
        }
    }

    fn handle_connect_rejection(&mut self, swarm: &mut Swarm<C>, peer: PeerId) {
        self.outbound_peers.remove(&peer);

        if self.is_enabled() {
            self.repair_outbound_peers(swarm);
        }
    }

    pub(crate) fn handle_failed_connect_request(
        &mut self,
        swarm: &mut Swarm<C>,
        request_id: OutboundRequestId,
    ) {
        if let Some(mut request_data) = self
            .controller
            .connect_request
            .remove_in_progress(&request_id)
        {
            if request_data.retry.count() < self.config.connect_request_max_retries {
                // Retry request after a delay
                request_data.retry.inc_count();

                self.controller
                    .connect_request
                    .add_to_queue(request_data.clone(), Some(request_data.retry.next_delay()));
            } else {
                // No more trials left
                error!(
                    "Failed to send connect request to {0} after {1} trials",
                    request_data.peer_id(),
                    request_data.retry.count(),
                );

                self.metrics.increment_total_failed_connect_requests();

                self.handle_connect_rejection(swarm, request_data.peer_id());
            }
        }
    }
}

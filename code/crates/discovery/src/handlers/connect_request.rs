use libp2p::{
    request_response::{OutboundRequestId, ResponseChannel},
    PeerId, Swarm,
};
use tracing::{debug, error, info, trace, warn};

use crate::{
    behaviour::{self, Response},
    request::RequestData,
    Discovery, DiscoveryClient,
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

        info!(
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

        if self.outbound_connections.contains_key(&peer) {
            info!("Peer {peer} is already an outbound connection");

            accepted = true;
        } else if self.inbound_connections.len() < self.config.num_inbound_peers {
            info!("Upgrading connection of peer {peer} to inbound connection");

            if let Some(connection_ids) = self.active_connections.get(&peer) {
                if connection_ids.len() > 1 {
                    // TODO: refer to `OutboundConnection` struct TODO in lib.rs
                    warn!("Peer {peer} has more than one connection");
                }
                match connection_ids.first() {
                    Some(connection_id) => {
                        debug!("Upgrading connection {connection_id} to inbound connection");
                        self.inbound_connections.insert(peer, *connection_id);
                    }
                    None => {
                        // This should not happen
                    }
                }
            }

            accepted = true;
        } else {
            info!("Rejecting connection upgrade of peer {peer} to inbound connection as the limit is reached");
        }

        self.update_connections_metrics();

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
            info!("Successfully upgraded connection of peer {peer} to outbound connection");

            if let Some(out_conn) = self.outbound_connections.get_mut(&peer) {
                out_conn.is_persistent = true;
            }

            // if all outbound connections are persistent, discovery is done
            if self
                .outbound_connections
                .values()
                .all(|out_conn| out_conn.is_persistent)
            {
                info!("All outbound connections are persistent");
                self.metrics.initial_discovery_finished();
                self.update_connections_metrics();
            }
        } else {
            info!("Peer {peer} rejected connection upgrade to outbound connection");

            self.metrics.increment_total_rejected_connect_requests();

            self.handle_connect_rejection(swarm, peer);
        }
    }

    fn handle_connect_rejection(&mut self, swarm: &mut Swarm<C>, peer: PeerId) {
        self.outbound_connections.remove(&peer);

        if self.is_enabled() {
            self.repair_outbound_connection(swarm);
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

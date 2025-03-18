use libp2p::{identify, swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, info, warn};

use crate::config::BootstrapProtocol;
use crate::{request::RequestData, Discovery, DiscoveryClient, OutboundConnection, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn handle_new_peer(
        &mut self,
        swarm: &mut Swarm<C>,
        connection_id: ConnectionId,
        peer_id: PeerId,
        info: identify::Info,
    ) {
        // Ignore identify intervals
        if self
            .active_connections
            .get(&peer_id)
            .is_some_and(|connections| connections.contains(&connection_id))
        {
            return;
        }

        if self
            .controller
            .dial
            .remove_in_progress(&connection_id)
            .is_none()
        {
            // Remove any matching in progress connections to avoid dangling data
            self.controller
                .dial_remove_matching_in_progress_connections(&peer_id);
        }

        match self.discovered_peers.insert(peer_id, info.clone()) {
            Some(_) => {
                info!(peer = %peer_id, "New connection from known peer");
            }
            None => {
                info!(peer = %peer_id, "Discovered peer");

                self.metrics.increment_total_discovered();

                // If the address belongs to a bootstrap node, save the peer id
                if let Some(bootstrap_node) = self
                    .bootstrap_nodes
                    .iter_mut()
                    .find(|(_, addr)| addr == info.listen_addrs.first().unwrap())
                {
                    *bootstrap_node = (Some(peer_id), info.listen_addrs.first().unwrap().clone());
                }
            }
        }

        if let Some(connection_ids) = self.active_connections.get_mut(&peer_id) {
            debug!(
                "Additional connection {connection_id} to peer {peer_id}, total connections: {}",
                connection_ids.len() + 1
            );

            connection_ids.push(connection_id);
        } else {
            self.active_connections.insert(peer_id, vec![connection_id]);
        }

        if self.is_enabled() {
            if self
                .outbound_connections
                .get(&peer_id)
                .is_some_and(|out_conn| out_conn.connection_id.is_none())
            {
                // This case happens when the peer was selected to be part of the outbound connections
                // but no connection was established yet. No need to trigger a connect request, it
                // was already done during the selection process.
                debug!(
                    peer = %peer_id, %connection_id,
                    "Connection is outbound (pending connect request)"
                );

                if let Some(out_conn) = self.outbound_connections.get_mut(&peer_id) {
                    out_conn.connection_id = Some(connection_id);
                }
            } else if self.state == State::Idle
                && self.outbound_connections.len() < self.config.num_outbound_peers
                // Not already an outbound connection
                && !self.outbound_connections.contains_key(&peer_id)
            {
                // If the initial discovery process is done and did not find enough peers,
                // the connection is outbound, otherwise it is ephemeral, except if later
                // the connection is requested to be persistent (inbound).
                debug!(
                    peer = %peer_id, %connection_id,
                    "Connection is outbound (incomplete initial discovery)"
                );

                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: None, // Will be set once the response is received
                        is_persistent: false,
                    },
                );

                self.controller
                    .connect_request
                    .add_to_queue(RequestData::new(peer_id), None);

                if self.outbound_connections.len() >= self.config.num_outbound_peers {
                    debug!(
                        count = self.outbound_connections.len(),
                        "Minimum number of peers reached"
                    );
                }
            } else {
                debug!(peer = %peer_id, %connection_id, "Connection is ephemeral");

                self.controller.close.add_to_queue(
                    (peer_id, connection_id),
                    Some(self.config.ephemeral_connection_timeout),
                );

                // Check if the re-extension dials are done
                if let State::Extending(_) = self.state {
                    self.make_extension_step(swarm);
                }
            }
            // Add the address to the Kademlia routing table
            if self.config.bootstrap_protocol == BootstrapProtocol::Kademlia {
                swarm
                    .behaviour_mut()
                    .add_address(&peer_id, info.listen_addrs.first().unwrap().clone());
            }
        } else {
            // If discovery is disabled, all connections are inbound. The
            // maximum number of inbound connections is enforced by the
            // corresponding parameter in the configuration.
            if self.inbound_connections.len() < self.config.num_inbound_peers {
                debug!(peer = %peer_id, %connection_id, "Connection is inbound");

                self.inbound_connections.insert(peer_id, connection_id);
            } else {
                warn!(peer = %peer_id, %connection_id, "Connections limit reached, refusing connection");

                self.controller
                    .close
                    .add_to_queue((peer_id, connection_id), None);
            }
        }

        self.update_connections_metrics();
    }
}

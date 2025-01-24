use libp2p::{identify, swarm::ConnectionId, PeerId, Swarm};
use tracing::{info, warn};

use crate::config::BootstrapProtocol;
use crate::{request::RequestData, Discovery, DiscoveryClient, OutboundConnection, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn is_bootstrap_node(&self, peer_id: &PeerId) -> bool {
        self.bootstrap_nodes
            .iter()
            .any(|(id, _)| id.as_ref() == Some(peer_id))
    }

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
                info!("New connection from known peer {peer_id}");
            }
            None => {
                info!("Discovered peer {peer_id}");

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
            warn!(
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
                info!("Connection {connection_id} from peer {peer_id} is outbound (pending connect request)");

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
                info!("Connection {connection_id} from peer {peer_id} is outbound (incomplete initial discovery)");

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
                    info!("Minimum number of peers reached");
                }
            } else {
                info!("Connection {connection_id} from peer {peer_id} is ephemeral");

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
            // If discovery is disabled, connections to bootstrap nodes are outbound,
            // and all other connections are ephemeral, except if later the connections
            // are requested to be persistent (inbound).
            if self.is_bootstrap_node(&peer_id) {
                info!("Connection {connection_id} from bootstrap node {peer_id} is outbound, requesting persistent connection");

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
            } else {
                info!("Connection {connection_id} from peer {peer_id} is ephemeral");

                self.controller.close.add_to_queue(
                    (peer_id, connection_id),
                    Some(self.config.ephemeral_connection_timeout),
                );
            }
        }

        self.update_connections_metrics();
    }
}

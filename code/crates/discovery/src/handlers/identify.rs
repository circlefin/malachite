use libp2p::{identify, swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, info, warn};

use crate::config::BootstrapProtocol;
use crate::OutboundState;
use crate::{request::RequestData, Discovery, DiscoveryClient, State};

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
    ) -> bool {
        // Return true every time another connection to the peer already exists.
        let mut is_already_connected = true;

        // Ignore identify intervals
        if self
            .active_connections
            .get(&peer_id)
            .is_some_and(|connection_ids| connection_ids.contains(&connection_id))
        {
            return is_already_connected;
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
                info!(
                    peer = %peer_id, %connection_id,
                    "New connection from known peer",
                );
            }
            None => {
                info!(
                    peer = %peer_id, %connection_id,
                    "Discovered peer",
                );

                self.metrics.increment_total_discovered();

                // If at least one listen address belongs to a bootstrap node, save the peer id
                if let Some(bootstrap_node) =
                    self.bootstrap_nodes.iter_mut().find(|(_, listen_addrs)| {
                        listen_addrs
                            .iter()
                            .any(|addr| info.listen_addrs.contains(addr))
                    })
                {
                    *bootstrap_node = (Some(peer_id), info.listen_addrs.clone());
                }
            }
        }

        if let Some(connection_ids) = self.active_connections.get_mut(&peer_id) {
            if connection_ids.len() >= self.config.max_connections_per_peer {
                warn!(
                    peer = %peer_id, %connection_id,
                    "Peer has has already reached the maximum number of connections ({}), closing connection",
                    self.config.max_connections_per_peer
                );

                self.controller
                    .close
                    .add_to_queue((peer_id, connection_id), None);

                return is_already_connected;
            } else {
                debug!(
                    peer = %peer_id, %connection_id,
                    "Additional connection to peer, total connections: {}",
                    connection_ids.len() + 1
                );
            }

            connection_ids.push(connection_id);
        } else {
            self.active_connections.insert(peer_id, vec![connection_id]);

            is_already_connected = false;
        }

        if self.is_enabled() {
            if self.outbound_peers.contains_key(&peer_id) {
                debug!(
                    peer = %peer_id, %connection_id,
                    "Connection is outbound"
                );
            } else if self.inbound_peers.contains(&peer_id) {
                debug!(
                    peer = %peer_id, %connection_id,
                    "Connection is inbound"
                );
            } else if self.state == State::Idle
                && self.outbound_peers.len() < self.config.num_outbound_peers
            {
                // If the initial discovery process is done and did not find enough peers,
                // the peer will be outbound, otherwise it is ephemeral, except if later
                // the peer is requested to be persistent (inbound).
                debug!(
                    peer = %peer_id, %connection_id,
                    "Connection is outbound (incomplete initial discovery)"
                );

                self.outbound_peers.insert(peer_id, OutboundState::Pending);

                self.controller
                    .connect_request
                    .add_to_queue(RequestData::new(peer_id), None);

                if self.outbound_peers.len() >= self.config.num_outbound_peers {
                    debug!(
                        count = self.outbound_peers.len(),
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
            // If discovery is disabled, all peers are inbound. The
            // maximum number of inbound peers is enforced by the
            // corresponding parameter in the configuration.
            if self.inbound_peers.len() < self.config.num_inbound_peers {
                debug!(peer = %peer_id, %connection_id, "Connection is inbound");

                self.inbound_peers.insert(peer_id);
            } else {
                warn!(peer = %peer_id, %connection_id, "Peers limit reached, refusing connection");

                self.controller
                    .close
                    .add_to_queue((peer_id, connection_id), None);

                // Set to true to avoid triggering new connection logic
                is_already_connected = true;
            }
        }

        self.update_discovery_metrics();

        return is_already_connected;
    }
}

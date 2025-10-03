use libp2p::{identify, swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, info, warn};

use crate::config::BootstrapProtocol;
use crate::OutboundState;
use crate::{request::RequestData, Discovery, DiscoveryClient, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    /// Update bootstrap node with peer_id if this peer matches a bootstrap node's addresses
    ///
    /// ## Bootstrap Discovery Flow:
    /// - Bootstrap configuration: bootstrap nodes configured with addresses but `peer_id = None`
    ///    ```rust
    ///    bootstrap_nodes:
    ///      [
    ///       (None, ["/ip4/1.2.3.4/tcp/8000", "/ip4/5.6.7.8/tcp/8000"]),
    ///       (None, ["/ip4/8.7.6.5/tcp/8000", "/ip4/4.3.2.1/tcp/8000"]),..
    ///      ]
    ///    ```
    /// - Initial dial: create `DialData` with `peer_id = None` and dial the **first** address
    ///    ```rust
    ///    DialData::new(None, vec![multiaddr]) // peer_id initially unknown
    ///    ```
    /// - Connection established: `handle_connection()` called with the actual `peer_id`
    ///    - Updates `dial_data.set_peer_id(peer_id)` via `dial_add_peer_id_to_dial_data()`
    /// - Identify protocol: peer sends identity information including supported protocols
    /// - Protocol check: only compatible peers reach `handle_new_peer()`
    /// - Bootstrap matching: **This function** matches the peer against bootstrap nodes:
    ///    - Check if peer's advertised addresses match any bootstrap node addresses
    ///    - If match found: update `bootstrap_nodes[i].0 = Some(peer_id)`
    ///
    /// Called after connection is established but before peer is added to active_connections
    fn update_bootstrap_node_peer_id(&mut self, peer_id: PeerId) {
        debug!(
            "Checking peer {} against {} bootstrap nodes",
            peer_id,
            self.bootstrap_nodes.len()
        );

        // Skip if peer is already identified (avoid duplicate work)
        if self
            .bootstrap_nodes
            .iter()
            .any(|(existing_peer_id, _)| existing_peer_id == &Some(peer_id))
        {
            debug!(
                "Peer {} already identified in bootstrap_nodes - skipping",
                peer_id
            );
            return;
        }

        // Find the dial_data that was updated in handle_connection
        // This dial_data originally had peer_id=None but now should have peer_id=Some(peer_id)
        let Some((_, dial_data)) = self
            .controller
            .dial
            .get_in_progress_iter()
            .find(|(_, dial_data)| dial_data.peer_id() == Some(peer_id))
        else {
            // This happens for incoming connections (peers that dialed this node)
            // since no dial_data was created for them
            return;
        };

        // Match dial addresses against bootstrap node configurations
        for (maybe_peer_id, listen_addrs) in self.bootstrap_nodes.iter_mut() {
            // Check if this bootstrap node is unidentified and addresses match
            if maybe_peer_id.is_none()
                && dial_data
                    .listen_addrs()
                    .iter()
                    .any(|dial_addr| listen_addrs.contains(dial_addr))
            {
                // Bootstrap discovery completed: None -> Some(peer_id)
                info!("Bootstrap peer {} successfully identified", peer_id);
                *maybe_peer_id = Some(peer_id);
                return;
            }
        }

        // This is only debug because some dialed peers (e.g. with discovery enabled)
        // are not one of the locally configured bootstrap nodes
        debug!("Failed to identify peer as bootstrap {}", peer_id);
    }

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

        // Match peer against bootstrap nodes
        self.update_bootstrap_node_peer_id(peer_id);

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

        is_already_connected
    }
}

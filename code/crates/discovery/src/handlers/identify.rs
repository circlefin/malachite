use libp2p::{identify, swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, info, warn};

use crate::{
    config::BootstrapProtocol, request::RequestData, util::strip_peer_id_from_multiaddr, Discovery,
    DiscoveryClient, OutboundState, State,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    /// Update bootstrap node with peer_id if this peer matches a bootstrap node's addresses
    ///
    /// ## Bootstrap Discovery Flow:
    /// - Bootstrap configuration: bootstrap nodes configured with addresses but `peer_id = None`
    ///    ```text
    ///    bootstrap_nodes:
    ///      [
    ///       (None, ["/ip4/1.2.3.4/tcp/8000", "/ip4/5.6.7.8/tcp/8000"]),
    ///       (None, ["/ip4/8.7.6.5/tcp/8000", "/ip4/4.3.2.1/tcp/8000"]),..
    ///      ]
    ///    ```
    /// - Initial dial: create `DialData` with `peer_id = None` and dial the **first** address
    ///    ```text
    ///    DialData::new(None, vec![multiaddr]) // peer_id initially unknown
    ///    ```
    /// - Connection established: `handle_connection()` called with the actual `peer_id`
    ///    - Updates `dial_data.set_peer_id(peer_id)` via `dial_add_peer_id_to_dial_data()`
    /// - Identify protocol: peer sends identity information including supported protocols
    /// - Protocol check: only compatible peers reach `handle_new_peer()`
    /// - Bootstrap matching: **This function** matches the peer against bootstrap nodes:
    ///    - For outbound: check addresses we dialed (from dial_data)
    ///    - For inbound: check actual connection remote address
    ///    - If match found: update `bootstrap_nodes[i].0 = Some(peer_id)`
    ///
    /// ## Note
    /// For inbound connections, we use the actual TCP connection remote address from
    /// `self.connections`, not the self-reported `identify::Info.listen_addrs`.
    /// This prevents address spoofing attacks where a malicious peer claims to be
    /// listening on a bootstrap node's address.
    ///
    /// Called after connection is established but before peer is added to active_connections
    fn update_bootstrap_node_peer_id(&mut self, connection_id: ConnectionId, peer_id: PeerId) {
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
        let dial_data = self
            .controller
            .dial
            .get_in_progress_iter()
            .find(|(_, dial_data)| dial_data.peer_id() == Some(peer_id))
            .map(|(_, dial_data)| dial_data);

        // Get the connection remote address for inbound connections
        // This prevents spoofing via self-reported identify addresses
        let connection_remote_addr = self.connections.get(&connection_id).map(|c| &c.remote_addr);

        // Match addresses against bootstrap node configurations
        // For outbound connections, check dial_data addresses (trusted since we initiated)
        // For inbound connections, check the connection remote address (trusted since it's TCP layer)
        for (maybe_peer_id, listen_addrs) in self.bootstrap_nodes.iter_mut() {
            // Check if this bootstrap node is unidentified
            if maybe_peer_id.is_some() {
                continue;
            }

            let addresses_match = if let Some(dial_data) = dial_data {
                // Outbound connection, check addresses we dialed
                dial_data
                    .listen_addrs()
                    .iter()
                    .any(|dial_addr| listen_addrs.contains(dial_addr))
            } else if let Some(remote_addr) = connection_remote_addr {
                // Inbound connection, check actual TCP remote address.
                // Strip /p2p/ component if present, since addresses may include a peer ID.
                // Note: This only matches if peer uses port reuse (source port == listen port).
                // With ephemeral source ports, peer gets identified as bootstrap when we
                // later dial their configured listen address.
                let remote_addr_stripped = strip_peer_id_from_multiaddr(remote_addr);
                listen_addrs.iter().any(|bootstrap_addr| {
                    let bootstrap_stripped = strip_peer_id_from_multiaddr(bootstrap_addr);
                    remote_addr_stripped == bootstrap_stripped
                })
            } else {
                // No connection info available, cannot verify
                debug!(
                    "No connection info for peer {} - cannot verify bootstrap status",
                    peer_id
                );
                false
            };

            if addresses_match {
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
        self.update_bootstrap_node_peer_id(connection_id, peer_id);

        if self.config.persistent_peers_only && !self.is_persistent_peer(&peer_id) {
            warn!(
                peer = %peer_id, %connection_id,
                "Rejecting connection from non-persistent peer as persistent_peers_only mode is on"
            );

            self.controller
                .close
                .add_to_queue((peer_id, connection_id), None);

            return is_already_connected;
        }

        // Remove from dial in-progress if this was an outbound connection we initiated.
        // For inbound connections, this will return None and we don't touch dial data.
        self.controller.dial.remove_in_progress(&connection_id);

        // Store signed peer record if available
        if let Some(envelope) = &info.signed_peer_record {
            self.signed_peer_records.insert(peer_id, envelope.clone());
            debug!(
                peer = %peer_id,
                "Stored signed peer record for secure peer exchange"
            );
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
                    "Peer has already reached the maximum number of connections ({}), closing connection",
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
                if let Some(addr) = info.listen_addrs.first() {
                    swarm.behaviour_mut().add_address(&peer_id, addr.clone());
                }
            }
        } else {
            // If discovery is disabled, classify based on actual connection direction
            let we_dialed = self
                .controller
                .dial
                .get_in_progress_iter()
                .any(|(_, dial_data)| dial_data.peer_id() == Some(peer_id))
                || self
                    .controller
                    .dial
                    .is_done_on(&crate::controller::PeerData::PeerId(peer_id));

            if we_dialed {
                // Accept all peers we dial (no capacity check)
                // When discovery is disabled, these are explicitly configured peers
                debug!(peer = %peer_id, %connection_id, "Connection is outbound");
                self.outbound_peers
                    .insert(peer_id, OutboundState::Confirmed);
            } else if self.inbound_peers.len() < self.config.num_inbound_peers {
                debug!(peer = %peer_id, %connection_id, "Connection is inbound");
                self.inbound_peers.insert(peer_id);
            } else {
                warn!(peer = %peer_id, %connection_id, "Inbound peers limit reached, refusing connection");
                self.controller
                    .close
                    .add_to_queue((peer_id, connection_id), None);
                is_already_connected = true;
            }
        }

        self.update_discovery_metrics();

        is_already_connected
    }
}

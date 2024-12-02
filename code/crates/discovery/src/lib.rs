// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::collections::{HashMap, HashSet};

use rand::seq::SliceRandom;
use tracing::{debug, error, info, trace, warn};

use malachite_metrics::Registry;

use libp2p::{
    core::ConnectedPoint,
    identify, kad,
    request_response::{self, OutboundRequestId},
    swarm::ConnectionId,
    Multiaddr, PeerId, Swarm,
};

mod util;

mod behaviour;
pub use behaviour::*;

mod connection;
use connection::ConnectionData;

mod config;
pub use config::Config;

mod controller;
use controller::{Controller, PeerData};

mod metrics;
use metrics::Metrics;

mod request;
use request::RequestData;

#[derive(Debug, PartialEq)]
enum State {
    Bootstrapping,
    Extending,
    Idle,
}

#[derive(Debug)]
struct OutboundConnection {
    connection_id: Option<ConnectionId>,
    is_persistent: bool,
}

#[derive(Debug)]
pub struct Discovery {
    config: Config,
    state: State,

    bootstrap_nodes: Vec<(Option<PeerId>, Multiaddr)>,
    discovered_peers: HashMap<PeerId, identify::Info>,
    active_connections: HashMap<PeerId, Vec<ConnectionId>>,
    outbound_connections: HashMap<PeerId, OutboundConnection>,
    inbound_connections: HashMap<PeerId, ConnectionId>,

    pub controller: Controller,
    metrics: Metrics,
}

impl Discovery {
    pub fn new(config: Config, bootstrap_nodes: Vec<Multiaddr>, registry: &mut Registry) -> Self {
        info!(
            "Discovery is {}",
            if config.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );

        let state = if config.enabled && bootstrap_nodes.is_empty() {
            warn!("No bootstrap nodes provided");
            info!("Discovery found 0 peers in 0ms");
            State::Idle
        } else if config.enabled {
            State::Bootstrapping
        } else {
            State::Idle
        };

        Self {
            config,
            state,

            bootstrap_nodes: bootstrap_nodes
                .clone()
                .into_iter()
                .map(|addr| (None, addr))
                .collect(),
            discovered_peers: HashMap::new(),
            active_connections: HashMap::new(),
            outbound_connections: HashMap::new(),
            inbound_connections: HashMap::new(),

            controller: Controller::new(),
            metrics: Metrics::new(registry, !config.enabled || bootstrap_nodes.is_empty()),
        }
    }

    /// ------------------------------------------------------------------------
    /// Dial and New Connections
    /// ------------------------------------------------------------------------

    pub fn can_dial(&self) -> bool {
        self.controller.dial.can_perform()
    }

    fn should_dial(
        &self,
        swarm: &Swarm<impl DiscoveryClient>,
        connection_data: &ConnectionData,
        check_already_dialed: bool,
    ) -> bool {
        connection_data.peer_id().as_ref().map_or(true, |id| {
            // Is not itself (peer id)
            id != swarm.local_peer_id()
            // Is not already connected
            && !swarm.is_connected(id)
        })
            // Has not already dialed, or has dialed but retries are allowed
            && (!check_already_dialed || !self.controller.dial_is_done_on(connection_data) || connection_data.retry.count() != 0)
            // Is not itself (multiaddr)
            && !swarm.listeners().any(|addr| *addr == connection_data.multiaddr())
    }

    pub fn dial_peer(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        connection_data: ConnectionData,
    ) {
        // Not checking if the peer was already dialed because it is done when
        // adding to the dial queue
        if !self.should_dial(swarm, &connection_data, false) {
            return;
        }

        let dial_opts = connection_data.build_dial_opts();
        let connection_id = dial_opts.connection_id();

        self.controller.dial_register_done_on(&connection_data);

        self.controller
            .dial
            .register_in_progress(connection_id, connection_data.clone());

        // Do not count retries as new interactions
        if connection_data.retry.count() == 0 {
            self.metrics.increment_total_dials();
        }

        info!(
            "Dialing peer at {}, retry #{}",
            connection_data.multiaddr(),
            connection_data.retry.count()
        );

        if let Err(e) = swarm.dial(dial_opts) {
            if let Some(peer_id) = connection_data.peer_id() {
                error!(
                    "Error dialing peer {} at {}: {}",
                    peer_id,
                    connection_data.multiaddr(),
                    e
                );
            } else {
                error!(
                    "Error dialing peer at {}: {}",
                    connection_data.multiaddr(),
                    e
                );
            }

            self.handle_failed_connection(swarm, connection_id);
        }
    }

    pub fn handle_connection(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
    ) {
        match endpoint {
            ConnectedPoint::Dialer { .. } => {
                debug!("Connected to {peer_id} with connection id {connection_id}");
            }
            ConnectedPoint::Listener { .. } => {
                debug!("Accepted incoming connection from {peer_id} with connection id {connection_id}");
            }
        }

        // Needed in case the peer was dialed without knowing the peer id
        self.controller
            .dial
            .register_done_on(PeerData::PeerId(peer_id));

        // This check is necessary to handle the case where two
        // nodes dial each other at the same time, which can lead
        // to a connection established (dialer) event for one node
        // after the connection established (listener) event on the
        // same node. Hence it is possible that the peer was already
        // added to the active connections.
        if self.active_connections.contains_key(&peer_id) {
            self.controller.dial.remove_in_progress(&connection_id);
            // Check the state to trigger potential next steps
            self.check_extension_status(swarm);
            return;
        }

        // Needed in case the peer was dialed without knowing the peer id
        self.controller
            .dial_add_peer_id_to_connection_data(connection_id, peer_id);
    }

    pub fn handle_failed_connection(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        connection_id: ConnectionId,
    ) {
        if let Some(mut connection_data) = self.controller.dial.remove_in_progress(&connection_id) {
            if connection_data.retry.count() < self.config.dial_max_retries {
                // Retry dialing after a delay
                connection_data.retry.inc_count();

                self.controller.dial.add_to_queue(
                    connection_data.clone(),
                    Some(connection_data.retry.next_delay()),
                );
            } else {
                // No more trials left
                error!(
                    "Failed to dial peer at {0} after {1} trials",
                    connection_data.multiaddr(),
                    connection_data.retry.count(),
                );

                self.metrics.increment_total_failed_dials();
                self.check_extension_status(swarm);
            }
        }
    }

    fn add_to_dial_queue(
        &mut self,
        swarm: &Swarm<impl DiscoveryClient>,
        connection_data: ConnectionData,
    ) {
        if self.should_dial(swarm, &connection_data, true) {
            // Already register as dialed address to avoid flooding the dial queue
            // with the same dial attempts.
            self.controller.dial_register_done_on(&connection_data);

            self.controller.dial.add_to_queue(connection_data, None);
        }
    }

    pub fn dial_bootstrap_nodes(&mut self, swarm: &Swarm<impl DiscoveryClient>) {
        for (peer_id, addr) in &self.bootstrap_nodes.clone() {
            self.add_to_dial_queue(swarm, ConnectionData::new(*peer_id, addr.clone()));
        }
    }

    /// ------------------------------------------------------------------------
    /// New Peer (Identified)
    /// ------------------------------------------------------------------------

    pub fn handle_new_peer(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        connection_id: ConnectionId,
        peer_id: PeerId,
        info: identify::Info,
    ) {
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
                "Additional connection to peer {peer_id}, total connections: {}",
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
                .map_or(false, |out_conn| out_conn.connection_id.is_none())
            {
                // This case happens when the peer was selected to be part of the outbound connections
                // but no connection was established yet. The connect request should still be done.
                info!("Connection from peer {peer_id} is outbound (pending connect request)");
                self.outbound_connections
                    .get_mut(&peer_id)
                    .map(|out_conn| out_conn.connection_id = Some(connection_id));
            } else if self.state == State::Idle
                && self.outbound_connections.len() < self.config.num_outbound_peers
            {
                // If the initial discovery process is done and did not find enough peers,
                // the connection is outbound, otherwise it is ephemeral, except if later
                // the connection is requested to be persistent (inbound).
                info!("Connection from peer {peer_id} is outbound (pending discovery extension)");
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: Some(connection_id),
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
                info!("Connection from peer {peer_id} is ephemeral");

                self.controller.close.add_to_queue(
                    (peer_id, connection_id),
                    Some(self.config.ephemeral_connection_timeout),
                );

                // Check if the re-extension dials are done
                if self.state == State::Extending {
                    self.check_extension_status(swarm);
                }
            }
            // Add the address to the Kademlia routing table
            swarm
                .behaviour_mut()
                .add_address(&peer_id, info.listen_addrs.first().unwrap().clone());
        } else {
            // If discovery is disabled, connections to bootstrap nodes are outbound,
            // and all other connections are ephemeral, except if later the connection
            // is requested to be persistent (inbound).
            if self.is_bootstrap_node(&peer_id) {
                info!("Connection from bootstrap node {peer_id} is outbound, requesting persistent connection");
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: Some(connection_id),
                        is_persistent: false,
                    },
                );

                self.controller
                    .connect_request
                    .add_to_queue(RequestData::new(peer_id), None);
            } else {
                info!("Connection from peer {peer_id} is ephemeral");

                self.controller.close.add_to_queue(
                    (peer_id, connection_id),
                    Some(self.config.ephemeral_connection_timeout),
                );
            }
        }

        self.update_connections_metrics(swarm);
    }

    /// ------------------------------------------------------------------------
    /// Peers Request
    /// ------------------------------------------------------------------------

    pub fn can_peers_request(&self) -> bool {
        self.is_enabled() && self.controller.peers_request.can_perform()
    }

    fn should_peers_request(
        &self,
        swarm: &Swarm<impl DiscoveryClient>,
        request_data: &RequestData,
    ) -> bool {
        // Is connected
        swarm.is_connected(&request_data.peer_id())
            // Has not already requested, or has requested but retries are allowed
            && (!self.controller.peers_request.is_done_on(&request_data.peer_id())
                || request_data.retry.count() != 0)
    }

    pub fn peers_request_peer(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        request_data: RequestData,
    ) {
        if !self.is_enabled() || !self.should_peers_request(swarm, &request_data) {
            return;
        }

        self.controller
            .peers_request
            .register_done_on(request_data.peer_id());

        // Do not count retries as new interactions
        if request_data.retry.count() == 0 {
            self.metrics.increment_total_peer_requests();
        }

        info!(
            "Requesting peers from peer {}, retry #{}",
            request_data.peer_id(),
            request_data.retry.count()
        );

        let request_id = swarm.behaviour_mut().send_request(
            &request_data.peer_id(),
            behaviour::Request::Peers(self.get_all_peers_except(request_data.peer_id())),
        );

        self.controller
            .peers_request
            .register_in_progress(request_id, request_data);
    }

    fn handle_failed_peers_request(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        request_id: OutboundRequestId,
    ) {
        if let Some(mut request_data) = self
            .controller
            .peers_request
            .remove_in_progress(&request_id)
        {
            if request_data.retry.count() < self.config.request_max_retries {
                // Retry request after a delay
                request_data.retry.inc_count();

                self.controller
                    .peers_request
                    .add_to_queue(request_data.clone(), Some(request_data.retry.next_delay()));
            } else {
                // No more trials left
                error!(
                    "Failed to send peers request to {0} after {1} trials",
                    request_data.peer_id(),
                    request_data.retry.count(),
                );

                self.metrics.increment_total_failed_peer_requests();
                self.check_extension_status(swarm);
            }
        }
    }

    fn process_received_peers(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        peers: HashSet<(Option<PeerId>, Multiaddr)>,
    ) {
        for (peer_id, listen_addr) in peers {
            self.add_to_dial_queue(swarm, ConnectionData::new(peer_id, listen_addr));
        }
    }

    /// ------------------------------------------------------------------------
    /// Connect Request
    /// ------------------------------------------------------------------------

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

    pub fn connect_request_peer(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        request_data: RequestData,
    ) {
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

    fn handle_failed_connect_request(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
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
                self.check_extension_status(swarm);
            }
        }
    }

    /// ------------------------------------------------------------------------
    /// Close Connection
    /// ------------------------------------------------------------------------

    pub fn can_close(&mut self) -> bool {
        self.state == State::Idle
    }

    fn should_close(&self, peer_id: PeerId, connection_id: ConnectionId) -> bool {
        self.outbound_connections
            .get(&peer_id)
            .map_or(true, |out_conn| {
                out_conn.connection_id != Some(connection_id)
            })
            && self.inbound_connections.get(&peer_id) != Some(&connection_id)
    }

    pub fn close_connection(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        if !self.should_close(peer_id, connection_id) {
            return;
        }

        if self
            .active_connections
            .get(&peer_id)
            .map_or(false, |connections| connections.contains(&connection_id))
        {
            if swarm.close_connection(connection_id) {
                info!("Closing connection {connection_id} to peer {peer_id}");
            } else {
                error!("Error closing connection to peer {peer_id}");
            }
        } else {
            warn!("Tried to close an unknown connection to peer {peer_id}: {connection_id}");
        }
    }

    pub fn handle_closed_connection(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        if let Some(connections) = self.active_connections.get_mut(&peer_id) {
            if connections.contains(&connection_id) {
                warn!("Removing active connection {connection_id} to peer {peer_id}");
                connections.retain(|id| id != &connection_id);
                if connections.is_empty() {
                    self.active_connections.remove(&peer_id);
                }
            } else {
                warn!("Non-established connection to peer {peer_id} closed: {connection_id}");
            }
        }

        // In case the connection was closed before identifying the peer
        self.controller.dial.remove_in_progress(&connection_id);

        if self
            .outbound_connections
            .get(&peer_id)
            .map_or(false, |out_conn| {
                out_conn.connection_id == Some(connection_id)
            })
        {
            warn!("Outbound connection to peer {peer_id} closed");
            self.outbound_connections.remove(&peer_id);
            self.update_connections_metrics(swarm);

            if self.is_enabled() {
                self.repair_outbound_connection(swarm);
            }
        } else if self.inbound_connections.get(&peer_id) == Some(&connection_id) {
            warn!("Inbound connection to peer {peer_id} closed");
            self.inbound_connections.remove(&peer_id);
            self.update_connections_metrics(swarm);
        } else {
            self.update_connections_metrics(swarm);
        }
    }

    /// ------------------------------------------------------------------------
    /// Peers Extension and Management
    /// ------------------------------------------------------------------------

    fn get_next_peer_to_peers_request(
        &self,
        swarm: &mut Swarm<impl DiscoveryClient>,
    ) -> Option<PeerId> {
        let mut furthest_peer_id = None;

        // Find the furthest peer in the routing table that has not been requested.
        // Both iterators do not implement trait `DoubleEndedIterator`,
        // so we cannot use `rev()` to directly start from the furthest peer
        for kbucket in swarm.behaviour_mut().kbuckets() {
            for entry in kbucket.iter() {
                let peer_id = entry.node.key.preimage().clone();
                if !self.controller.peers_request.is_done_on(&peer_id) {
                    furthest_peer_id = Some(peer_id);
                }
            }
        }

        furthest_peer_id
    }

    fn select_outbound_connections(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        let kbuckets: Vec<(u32, Vec<PeerId>)> = self.get_routing_table(swarm);

        // Cannot select more outbound connections than the number of active connections
        let mut new_outbound_connections = std::cmp::min(
            self.config.num_outbound_peers - self.outbound_connections.len(),
            self.active_connections_len(),
        );

        debug!(
            "Selecting {} outbound connections",
            new_outbound_connections
        );

        // select randomly num_outbound_peers from the routing table
        // NOTE: theorically, it would be better to select at least one peer from each bucket
        // to ensure a good distribution of peers in the network. However, since we are considering
        // a relatively small network (about thousands of peers), we can select randomly without
        // affecting the distribution of peers in the network.
        let mut rng = rand::thread_rng();
        let mut available_peers: Vec<PeerId> = kbuckets
            .iter()
            .flat_map(|kbucket| kbucket.1.clone())
            // Remove already selected outbound connections
            .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
            // Remove peers to which a connect request has already been done
            .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
            .collect();

        if available_peers.len() < new_outbound_connections {
            warn!(
                "Not enough available peers in routing table ({}) to select {} outbound connections, checking on active connections to complete",
                available_peers.len(),
                new_outbound_connections
            );

            let num_missing_peers = new_outbound_connections - available_peers.len();
            let available_active_peers: Vec<PeerId> = self
                .active_connections
                .keys()
                .filter(|peer_id| !available_peers.contains(peer_id))
                .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
                .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
                .cloned()
                .collect();

            if available_active_peers.len() < num_missing_peers {
                warn!(
                    "Not enough available peers in active connections ({}) to select {} outbound connections",
                    available_active_peers.len(),
                    num_missing_peers
                )
            }

            new_outbound_connections = std::cmp::min(
                available_peers.len() + available_active_peers.len(),
                new_outbound_connections,
            );

            available_peers.extend(available_active_peers);
        }

        let selected_peers: Vec<PeerId> = available_peers
            .choose_multiple(&mut rng, new_outbound_connections)
            .cloned()
            .collect();

        for peer_id in selected_peers {
            if let Some(connection_ids) = self.active_connections.get(&peer_id) {
                if connection_ids.len() > 1 {
                    warn!("Peer {peer_id} has more than one connection");
                    // TODO: to avoid any issues, one would need to make a PR to rust-libp2p
                    // to include the connection id in the request_response protocol.
                    // Now, it is impossible to know which connection was used to send the request;
                    // hence, which connection should be considered as the outbound connection.
                    // For now, we just take the first connection id.
                }
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: connection_ids.first().cloned(),
                        is_persistent: false,
                    },
                );
            } else {
                warn!("Peer {peer_id} has no active connection");
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: None,
                        is_persistent: false,
                    },
                );
            }
        }

        // Make sure that the inbound connections are not part of the outbound connections
        self.inbound_connections.retain(|peer_id, connection_id| {
            self.outbound_connections
                .get(peer_id)
                .map_or(true, |out_conn| {
                    out_conn.connection_id != Some(*connection_id)
                })
        });

        info!("Selected outbound connections, requesting persistent connections to {} non-persistent connections",
            self
            .outbound_connections
            .iter()
            .filter(|(_, out_conn)| !out_conn.is_persistent).count()
        );
        for (peer_id, _) in self
            .outbound_connections
            .iter()
            .filter(|(_, out_conn)| !out_conn.is_persistent)
        {
            self.controller
                .connect_request
                .add_to_queue(RequestData::new(*peer_id), None);
        }
    }

    fn repair_outbound_connection(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() || self.outbound_connections.len() >= self.config.num_outbound_peers {
            return;
        }

        info!("Reparing outbound connections");

        if let Some((peer_id, connection_id)) = self
            .inbound_connections
            .iter()
            // Do not select inbound connections whose peer id is already in the outbound connections
            // with another connection id
            .find(|(peer_id, _)| !self.outbound_connections.contains_key(peer_id))
            .map(|(peer_id, connection_id)| (*peer_id, *connection_id))
        {
            // Upgrade any inbound connection to outbound if any is available
            info!("Upgrading connection of peer {peer_id} to outbound connection");
            self.inbound_connections.remove(&peer_id);
            self.outbound_connections.insert(
                peer_id,
                OutboundConnection {
                    connection_id: Some(connection_id),
                    is_persistent: true, // persistent connection already established
                },
            );

            // Consider the connect request as done
            self.controller.connect_request.register_done_on(peer_id);

            self.update_connections_metrics(swarm);
        } else {
            // select anyone in the routing table
            let available_peers: Vec<PeerId> = self
                .get_routing_table(swarm)
                .iter()
                .flat_map(|kbucket| kbucket.1.clone())
                // Remove already selected outbound connections
                .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
                .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
                .collect();
            if let Some(new_peer_id) = available_peers.iter().next() {
                info!("Trying to connect to peer {new_peer_id} to repair outbound connections");
                if let Some(connection_ids) = self.active_connections.get(new_peer_id) {
                    if connection_ids.len() > 1 {
                        warn!("Peer {new_peer_id} has more than one connection");
                    }
                    self.outbound_connections.insert(
                        *new_peer_id,
                        OutboundConnection {
                            connection_id: connection_ids.first().cloned(),
                            is_persistent: false,
                        },
                    );
                } else {
                    warn!("Peer {new_peer_id} has no active connection");
                    self.outbound_connections.insert(
                        *new_peer_id,
                        OutboundConnection {
                            connection_id: None,
                            is_persistent: false,
                        },
                    );
                }
                // Request the new peer to be part of its inbound connections
                self.controller
                    .connect_request
                    .add_to_queue(RequestData::new(*new_peer_id), None);
            } else {
                // If no peer is available in the routing table, then look in the active connections
                let available_peers: Vec<PeerId> = self
                    .active_connections
                    .keys()
                    .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
                    .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
                    .cloned()
                    .collect();
                if let Some(new_peer_id) = available_peers.iter().next() {
                    info!("Trying to connect to peer {new_peer_id} to repair outbound connections");
                    if let Some(connection_ids) = self.active_connections.get(new_peer_id) {
                        if connection_ids.len() > 1 {
                            warn!("Peer {new_peer_id} has more than one connection");
                        }
                        self.outbound_connections.insert(
                            *new_peer_id,
                            OutboundConnection {
                                connection_id: connection_ids.first().cloned(),
                                is_persistent: false,
                            },
                        );

                        // Request the new peer to be part of its inbound connections
                        self.controller
                            .connect_request
                            .add_to_queue(RequestData::new(*new_peer_id), None);
                    }
                } else {
                    // This is very unlikely to happen, but it is possible if the network is too small
                    // or the outbound and inbound parameters are too restrictive.
                    warn!("No available peers to repair outbound connections, triggering discovery extension");
                    self.state = State::Extending;
                    self.check_extension_status(swarm); // trigger extension
                }
            }
        }
    }

    fn adjust_connections(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        info!("Adjusting connections");
        self.select_outbound_connections(swarm);

        let connections_to_close: Vec<(PeerId, ConnectionId)> = self
            .active_connections
            .iter()
            .flat_map(|(peer_id, connection_ids)| {
                connection_ids
                    .iter()
                    .map(|connection_id| (*peer_id, connection_id.clone()))
            })
            // Filter out the connections that are already in the inbound connections
            .filter(|(peer_id, connection_id)| {
                self.inbound_connections
                    .get(peer_id)
                    .map_or(true, |in_conn_id| *in_conn_id != *connection_id)
            })
            // Filter out the connections that are already in the outbound connections
            .filter(|(peer_id, connection_id)| {
                self.outbound_connections
                    .get(peer_id)
                    .map_or(true, |out_conn| {
                        out_conn.connection_id != Some(*connection_id)
                    })
            })
            .collect();

        info!(
            "Connections adjusted by disconnecting {} peers, keeping outbound peers: {:?}, and inbound peers: {:?}",
            connections_to_close.len(),
            self.outbound_connections,
            self.inbound_connections
        );

        for (peer_id, connection_id) in connections_to_close {
            self.controller.close.add_to_queue(
                (peer_id, connection_id),
                Some(self.config.ephemeral_connection_timeout),
            );
        }
    }

    fn check_extension_status(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() || self.state != State::Extending {
            return;
        }

        let (is_idle, pending_connections_len, pending_peers_requests_len) =
            self.controller.is_idle();
        let rx_dial_len = self.controller.dial.queue_len();
        let rx_peers_request_len = self.controller.peers_request.queue_len();

        if is_idle && rx_dial_len == 0 && rx_peers_request_len == 0 {
            info!("Number of active connections not connect requested: {}, outbound connections: {}, inbound connections: {}",
                self.active_connections
                    .iter()
                    .filter(|(peer_id, _)| !self.controller.connect_request.is_done_on(peer_id))
                    .count(),
                self.outbound_connections.len(),
                self.inbound_connections.len(),
            );

            // Done when we found enough peers to which we did not request persistent connection yet
            // to potentially upgrade them to outbound connections we are missing.
            if self
                .active_connections
                .iter()
                .filter(|(peer_id, _)| !self.controller.connect_request.is_done_on(peer_id))
                .count()
                < (self.config.num_outbound_peers - self.outbound_connections.len())
            {
                if let Some(peer_id) = self.get_next_peer_to_peers_request(swarm) {
                    info!(
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
                self.active_connections_len(),
                self.config.num_outbound_peers,
                self.metrics.elapsed().as_millis()
            );

            self.adjust_connections(swarm);

            self.state = State::Idle;
        } else {
            info!("Discovery extension in progress ({}ms), {} pending connections ({} in channel), {} pending requests ({} in channel)",
                self.metrics.elapsed().as_millis(),
                pending_connections_len,
                rx_dial_len,
                pending_peers_requests_len,
                rx_peers_request_len,
            );
        }
    }

    /// ------------------------------------------------------------------------
    /// Network Events
    /// ------------------------------------------------------------------------

    pub fn on_network_event(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        network_event: behaviour::NetworkEvent,
    ) {
        match network_event {
            behaviour::NetworkEvent::Kademlia(event) => match event {
                kad::Event::RoutingUpdated { .. } => {
                    self.print_stats(swarm);
                }

                kad::Event::OutboundQueryProgressed { result, step, .. } => match result {
                    kad::QueryResult::Bootstrap(Ok(_)) => {
                        if step.last {
                            // NOTE: A new bootstrap query is initiated every time a new peer is added
                            // to the routing table (delayed with kad crate configuration parameter `automatic_bootstrap_throttle`
                            // to avoid multiple queries in a short period of time).
                            // Here, we only consider the first bootstrap query to determine the status of the discovery process.
                            if self.state == State::Bootstrapping {
                                info!(
                                    "Discovery bootstrap done in {}ms, found {} peers",
                                    self.metrics.elapsed().as_millis(),
                                    self.active_connections_len()
                                );

                                if self.active_connections_len() < self.config.num_outbound_peers {
                                    info!("Initiating discovery extension");

                                    self.state = State::Extending;
                                    self.check_extension_status(swarm); // trigger extension
                                } else {
                                    info!(
                                        "Discovery found {} peers (expected {}) in {}ms",
                                        self.active_connections_len(),
                                        self.config.num_outbound_peers,
                                        self.metrics.elapsed().as_millis()
                                    );

                                    self.adjust_connections(swarm);

                                    self.state = State::Idle;
                                }
                            }
                        }
                    }

                    kad::QueryResult::Bootstrap(Err(error)) => {
                        error!("Bootstrap failed: {error}");

                        self.state = State::Idle;
                    }

                    _ => {}
                },

                _ => {}
            },

            behaviour::NetworkEvent::RequestResponse(event) => {
                match event {
                    request_response::Event::Message {
                        peer,
                        message:
                            request_response::Message::Request {
                                request, channel, ..
                            },
                    } => match request {
                        behaviour::Request::Peers(peers) => {
                            debug!(peer_id = %peer, "Received request for peers from peer");

                            // Compute the difference between the known peers and the requested peers
                            // to avoid sending the requesting peer the peers it already knows.
                            let peers_difference = self
                                .get_all_peers_except(peer)
                                .difference(&peers)
                                .cloned()
                                .collect();

                            if swarm
                                .behaviour_mut()
                                .send_response(
                                    channel,
                                    behaviour::Response::Peers(peers_difference),
                                )
                                .is_err()
                            {
                                error!("Error sending peers to {peer}");
                            } else {
                                trace!("Sent peers to {peer}");
                            }
                        }

                        behaviour::Request::Connect() => {
                            debug!(peer_id = %peer, "Received connect request from peer");

                            let mut response: bool = false;

                            if self.outbound_connections.contains_key(&peer) {
                                info!("Peer {peer} is already an outbound connection");

                                response = true;
                            } else if self.inbound_connections.len() < self.config.num_inbound_peers
                            {
                                info!("Upgrading connection of peer {peer} to inbound connection");

                                if let Some(connection_ids) = self.active_connections.get(&peer) {
                                    if connection_ids.len() > 1 {
                                        warn!("Peer {peer} has more than one connection");
                                    }
                                    match connection_ids.first() {
                                        Some(connection_id) => {
                                            self.inbound_connections.insert(peer, *connection_id);
                                        }
                                        None => {
                                            // This should not happen
                                        }
                                    }
                                }

                                self.update_connections_metrics(swarm);

                                response = true;
                            } else {
                                info!("Rejecting connection upgrade of peer {peer} to inbound connection as the limit is reached");
                            }

                            if swarm
                                .behaviour_mut()
                                .send_response(channel, behaviour::Response::Connect(response))
                                .is_err()
                            {
                                error!("Error sending connect response to {peer}");
                            } else {
                                trace!("Sent connect response to {peer}");
                            }
                        }
                    },

                    request_response::Event::Message {
                        peer,
                        message:
                            request_response::Message::Response {
                                response,
                                request_id,
                                ..
                            },
                    } => match response {
                        behaviour::Response::Peers(peers) => {
                            debug!(count = peers.len(), peer_id = %peer, "Received peers");

                            self.controller
                                .peers_request
                                .remove_in_progress(&request_id);

                            self.process_received_peers(swarm, peers);
                            self.check_extension_status(swarm);
                        }

                        behaviour::Response::Connect(accepted) => {
                            debug!(peer_id = %peer, accepted, "Received connect response from peer");

                            self.controller
                                .connect_request
                                .remove_in_progress(&request_id);

                            if accepted {
                                info!("Successfully upgraded connection of peer {peer} to outbound connection");

                                self.outbound_connections.get_mut(&peer).map(|out_conn| {
                                    out_conn.is_persistent = true;
                                });

                                // if all outbound connections are persistent, discovery is done
                                if self
                                    .outbound_connections
                                    .values()
                                    .all(|out_conn| out_conn.is_persistent)
                                {
                                    info!("All outbound connections are persistent");
                                    self.metrics.initial_discovery_finished();
                                    self.print_stats(swarm);
                                }
                            } else {
                                info!("Peer {peer} rejected connection upgrade to outbound connection");

                                self.outbound_connections.remove(&peer);
                                self.repair_outbound_connection(swarm);
                            }
                        }
                    },

                    request_response::Event::OutboundFailure {
                        peer,
                        request_id,
                        error,
                    } => {
                        error!("Outbound request to {peer} failed: {error}");
                        if self.controller.peers_request.is_in_progress(&request_id) {
                            self.handle_failed_peers_request(swarm, request_id);
                        } else if self.controller.connect_request.is_in_progress(&request_id) {
                            self.handle_failed_connect_request(swarm, request_id);
                        } else {
                            // This should not happen
                            error!("Unknown outbound request failure to {peer}");
                        }
                    }

                    _ => {}
                }
            }
        }
    }

    /// ------------------------------------------------------------------------
    /// Helper Functions
    /// ------------------------------------------------------------------------

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn update_connections_metrics(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        let num_active_connections = self.active_connections_len();
        let num_outbound_connections = self.outbound_connections.len();
        let num_inbound_connections = self.inbound_connections.len();
        let num_ephemeral_connections = num_active_connections
            .saturating_sub(num_outbound_connections + num_inbound_connections);

        info!(
            "Active connections: {}, Outbound connections: {}, Inbound connections: {}, Ephemeral connections: {}",
            num_active_connections,
            num_outbound_connections,
            num_inbound_connections,
            num_ephemeral_connections,
        );

        let equation_is_satisfied = num_active_connections
            == num_outbound_connections + num_inbound_connections + num_ephemeral_connections;

        if !equation_is_satisfied {
            error!("The number of active connections does not match the sum of outbound, inbound, and ephemeral connections");
        }

        self.metrics.set_connections_status(
            num_active_connections,
            num_outbound_connections,
            num_inbound_connections,
            num_ephemeral_connections,
        );

        self.print_stats(swarm);
    }

    fn is_bootstrap_node(&self, peer_id: &PeerId) -> bool {
        self.bootstrap_nodes
            .iter()
            .any(|(id, _)| id.as_ref() == Some(peer_id))
    }

    /// Returns all discovered peers, including bootstrap nodes, except the given peer.
    fn get_all_peers_except(&self, peer: PeerId) -> HashSet<(Option<PeerId>, Multiaddr)> {
        let mut remaining_bootstrap_nodes: Vec<_> = self.bootstrap_nodes.clone();

        let mut peers: HashSet<_> = self
            .discovered_peers
            .iter()
            .filter_map(|(peer_id, info)| {
                if peer_id == &peer {
                    // Remove the peer also from the bootstrap nodes (if it is there)
                    info.listen_addrs.first().map(|addr| {
                        remaining_bootstrap_nodes.retain(|(_, x)| x != addr);
                    });

                    return None;
                }

                info.listen_addrs.first().map(|addr| {
                    remaining_bootstrap_nodes.retain(|(_, x)| x != addr);
                    (Some(*peer_id), addr.clone())
                })
            })
            .collect();

        for (peer_id, addr) in remaining_bootstrap_nodes {
            peers.insert((peer_id, addr));
        }

        peers
    }

    fn get_routing_table(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
    ) -> Vec<(u32, Vec<PeerId>)> {
        let mut kbuckets: Vec<(u32, Vec<PeerId>)> = Vec::new();
        for kbucket in swarm.behaviour_mut().kbuckets() {
            let peers = kbucket
                .iter()
                .map(|entry| entry.node.key.preimage().clone())
                .collect();
            let index = kbucket.range().0.ilog2().unwrap_or(0);
            kbuckets.push((index, peers));
        }

        kbuckets
    }

    fn print_stats(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() {
            return;
        }

        let kbuckets: Vec<(u32, Vec<PeerId>)> = self.get_routing_table(swarm);

        let mut json = serde_json::json!({
            "time": self.metrics.initial_discovery_duration().as_millis(),
            "localPeerId": swarm.local_peer_id(),
            "totalPeers": kbuckets.iter().map(|(_, peers)| peers.len()).sum::<usize>(),
            "subset": self.outbound_connections.keys().cloned().collect::<Vec<_>>(),
            "kbuckets": {}
        });

        for (index, peers) in kbuckets {
            json["kbuckets"][index.to_string()] = serde_json::to_value(peers).unwrap();
        }

        println!("{}", serde_json::to_string(&json).unwrap());
    }

    fn active_connections_len(&self) -> usize {
        self.active_connections.values().map(Vec::len).sum()
    }
}

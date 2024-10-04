// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tokio::{sync::mpsc, time::sleep};
use tracing::{self as _, debug, error, info, trace};

use libp2p::{
    core::ConnectedPoint,
    identify,
    request_response::{self, OutboundRequestId},
    swarm::ConnectionId,
    Multiaddr, PeerId, Swarm,
};

mod behaviour;
pub use behaviour::*;

mod connection;
pub use connection::*;

const DISCOVERY_PROTOCOL: &str = "/malachite-discover/v1beta1";

#[derive(Debug)]
pub struct Discovery {
    peers: HashMap<PeerId, identify::Info>,
    is_enabled: bool,
    tx_dial: mpsc::UnboundedSender<ConnectionData>,
    is_done: bool,
    bootstrap_nodes: Vec<Multiaddr>,
    dialed_peer_ids: HashSet<PeerId>,
    dialed_multiaddrs: HashSet<Multiaddr>,
    pending_connections: HashMap<ConnectionId, ConnectionData>,
    connections_types: HashMap<PeerId, ConnectionType>,
    requested_peer_ids: HashSet<PeerId>,
    pending_requests: HashSet<OutboundRequestId>,
    /// Performance metrics
    total_interactions: usize,
    total_interactions_failed: usize,
    start_time: Instant,
    duration: Duration,
}

impl Discovery {
    pub fn new(
        enable_discovery: bool,
        tx_dial: mpsc::UnboundedSender<ConnectionData>,
        bootstrap_nodes: Vec<Multiaddr>,
    ) -> Self {
        Discovery {
            peers: HashMap::new(),
            is_enabled: enable_discovery,
            tx_dial,
            is_done: false,
            bootstrap_nodes,
            dialed_peer_ids: HashSet::new(),
            dialed_multiaddrs: HashSet::new(),
            pending_connections: HashMap::new(),
            connections_types: HashMap::new(),
            requested_peer_ids: HashSet::new(),
            pending_requests: HashSet::new(),
            total_interactions: 0,
            total_interactions_failed: 0,
            start_time: Instant::now(),
            duration: Duration::default(),
        }
    }

    pub fn remove_peer(&mut self, peer_id: PeerId) {
        self.peers.remove(&peer_id);
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn handle_failed_connection(&mut self, connection_id: ConnectionId) {
        if self.is_enabled {
            if let Some(mut connection_data) = self.pending_connections.get(&connection_id).cloned()
            {
                self.pending_connections.remove(&connection_id);

                if connection_data.get_trial() < DIAL_MAX_TRIALS {
                    connection_data.increment_trial();

                    let tx_dial = self.tx_dial.clone();
                    tokio::spawn(async move {
                        sleep(connection_data.next_delay()).await;
                        tx_dial.send(connection_data).unwrap_or_else(|e| {
                            error!("Error sending dial request to channel: {e}");
                        });
                    });
                } else {
                    error!(
                        "Failed to dial peer at {0} after {1} trials",
                        connection_data.multiaddr,
                        connection_data.get_trial(),
                    );
                    self.total_interactions_failed += 1;
                    self.check_if_done();
                }
            }
        }
    }

    fn register_failed_request(&mut self, request_id: OutboundRequestId) {
        if self.is_enabled {
            self.pending_requests.remove(&request_id);
            self.total_interactions_failed += 1;
        }
    }

    pub fn dial_peer(
        &mut self,
        swarm: &mut Swarm<impl SendResponse>,
        connection_data: ConnectionData,
    ) {
        let ConnectionData {
            peer_id, multiaddr, ..
        } = connection_data.clone();
        let trial = connection_data.get_trial();

        if peer_id.as_ref().map_or(false, |id| {
            // Is itself
            id == swarm.local_peer_id()
            // Is already connected
            || swarm.is_connected(id)
            // Has already been dialed (but ok if retrying)
            || (self.dialed_peer_ids.contains(id) && trial == 1)
        })
            // Has already been dialed (but ok if retrying)
            || (self.dialed_multiaddrs.contains(&multiaddr) && trial == 1)
            // Is itself
            || swarm.listeners().any(|addr| *addr == multiaddr)
        {
            return;
        }

        let dial_opts = connection_data.build_dial_opts();
        let connection_id = dial_opts.connection_id();

        if let Some(peer_id) = peer_id.as_ref() {
            self.dialed_peer_ids.insert(peer_id.clone());
        }
        self.dialed_multiaddrs.insert(multiaddr.clone());

        self.pending_connections
            .insert(connection_id, connection_data);

        if trial == 1 {
            self.total_interactions += 1;
        }

        if let Err(e) = swarm.dial(dial_opts) {
            if let Some(peer_id) = peer_id {
                error!("Error dialing peer {peer_id}: {e}");
            } else {
                error!("Error dialing peer {multiaddr}: {e}");
            }
            self.handle_failed_connection(connection_id);
        }
    }

    pub fn handle_connection(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
    ) {
        if self.is_enabled {
            match endpoint {
                ConnectedPoint::Dialer { .. } => {
                    debug!("Connected to {peer_id}");
                }
                ConnectedPoint::Listener { .. } => {
                    debug!("Accepted incoming connection from {peer_id}");
                }
            }

            if !self.connections_types.contains_key(&peer_id) {
                self.connections_types
                    .insert(peer_id.clone(), endpoint.into());
            }

            self.pending_connections.remove(&connection_id);
            // This call is necessary to record the peer id of a
            // bootstrap node (which was unknown before)
            self.dialed_peer_ids.insert(peer_id.clone());
            // This check is necessary to handle the case where two
            // nodes dial each other at the same time, which can lead
            // to a connection established (dialer) event for one node
            // after the connection established (listener) event on the
            // same node. Hence it is possible that the request for
            // peers was already sent before this event.
            if self.requested_peer_ids.contains(&peer_id) {
                self.check_if_done();
            }
        }
    }

    /// Returns all known peers, including bootstrap nodes, except the given peer.
    fn get_all_peers_except(&self, peer: PeerId) -> HashSet<(Option<PeerId>, Multiaddr)> {
        let mut remaining_bootstrap_nodes: Vec<_> = self.bootstrap_nodes.clone();

        let mut peers: HashSet<_> = self
            .peers
            .iter()
            .filter_map(|(peer_id, info)| {
                if peer_id == &peer {
                    return None;
                }

                info.listen_addrs.get(0).map(|addr| {
                    remaining_bootstrap_nodes.retain(|x| x != addr);
                    (Some(peer_id.clone()), addr.clone())
                })
            })
            .collect();

        for addr in remaining_bootstrap_nodes {
            peers.insert((None, addr));
        }

        peers
    }

    pub fn handle_new_peer(
        &mut self,
        behaviour: Option<&mut behaviour::Behaviour>,
        peer_id: PeerId,
        info: identify::Info,
    ) {
        if self.is_enabled
            && !self.peers.contains_key(&peer_id)
            // Only request when the peer initiated the connection
            && self.connections_types.get(&peer_id) == Some(&ConnectionType::Dial)
        {
            if let Some(request_response) = behaviour {
                debug!("Requesting peers from {peer_id}");

                let request_id = request_response.send_request(
                    &peer_id,
                    behaviour::Request::Peers(self.get_all_peers_except(peer_id)),
                );
                self.requested_peer_ids.insert(peer_id.clone());
                self.pending_requests.insert(request_id);
            } else {
                // This should never happen
                error!("Discovery is enabled but request-response is not available");
            }
        }

        self.connections_types.remove(&peer_id);

        self.peers.insert(peer_id, info);
        self.check_if_done();
    }

    pub fn check_if_done(&mut self) -> bool {
        if !self.is_enabled {
            return false;
        }
        if self.is_done {
            return true;
        }

        if self.pending_connections.is_empty() && self.pending_requests.is_empty() {
            self.is_done = true;
            self.duration = self.start_time.elapsed();
            info!(
                "Discovery finished in {}ms, found {} peers, dialed {} peers, {} successful, {} failed",
                self.duration.as_millis(),
                self.peers.len(),
                self.total_interactions,
                self.total_interactions - self.total_interactions_failed,
                self.total_interactions_failed,
            );
            return true;
        }

        info!(
            "Discovery in progress, {} pending connections, {} pending requests",
            self.pending_connections.len(),
            self.pending_requests.len(),
        );

        false
    }

    fn process_received_peers(
        &mut self,
        swarm: &mut Swarm<impl SendResponse>,
        peers: HashSet<(Option<PeerId>, Multiaddr)>,
    ) {
        // TODO check upper bound on number of peers
        for (peer_id, listen_addr) in peers {
            self.dial_peer(swarm, ConnectionData::new(peer_id, listen_addr));
        }
    }

    pub fn on_event(&mut self, event: behaviour::Event, swarm: &mut Swarm<impl SendResponse>) {
        match event {
            behaviour::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
            } => match request {
                behaviour::Request::Peers(peers) => {
                    debug!("Received request for peers from {peer}");

                    // Compute the difference between the known peers and the requested peers
                    // to avoid sending the requesting peer the peers it already knows.
                    let peers_difference = self
                        .get_all_peers_except(peer)
                        .difference(&peers)
                        .cloned()
                        .collect();

                    if swarm
                        .behaviour_mut()
                        .send_response(channel, behaviour::Response::Peers(peers_difference))
                        .is_err()
                    {
                        error!("Error sending peers to {peer}");
                    } else {
                        trace!("Sent peers to {peer}");
                    }

                    self.process_received_peers(swarm, peers);
                }
            },

            behaviour::Event::Message {
                peer,
                message:
                    request_response::Message::Response {
                        response,
                        request_id,
                        ..
                    },
            } => match response {
                behaviour::Response::Peers(peers) => {
                    self.pending_requests.remove(&request_id);
                    debug!("Received {} peers from {peer}", peers.len());

                    self.process_received_peers(swarm, peers);
                    self.check_if_done();
                }
            },

            behaviour::Event::OutboundFailure {
                request_id,
                peer,
                error,
            } => {
                error!("Outbound request to {peer} failed: {error}");
                self.register_failed_request(request_id);
                self.check_if_done();
            }

            _ => {}
        }
    }
}

// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use malachite_metrics::Registry;

use libp2p::{
    core::ConnectedPoint,
    identify,
    request_response::{self, OutboundRequestId},
    swarm::ConnectionId,
    Multiaddr, PeerId, Swarm,
};

mod util;

mod behaviour;
pub use behaviour::*;

mod connection;
pub use connection::ConnectionData;
use connection::ConnectionType;

mod config;
pub use config::Config;

mod handler;
use handler::Handler;

mod metrics;
use metrics::Metrics;

const DISCOVERY_PROTOCOL: &str = "/malachite-discovery/v1beta1";

#[derive(Debug)]
pub struct Discovery {
    config: Config,
    peers: HashMap<PeerId, identify::Info>,
    bootstrap_nodes: Vec<Multiaddr>,
    tx_dial: mpsc::UnboundedSender<ConnectionData>,
    handler: Handler,
    metrics: Metrics,
}

impl Discovery {
    pub fn new(
        config: Config,
        tx_dial: mpsc::UnboundedSender<ConnectionData>,
        bootstrap_nodes: Vec<Multiaddr>,
        registry: &mut Registry,
    ) -> Self {
        info!(
            "Discovery is {}",
            if config.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );

        Self {
            config,
            peers: HashMap::new(),
            bootstrap_nodes,
            tx_dial,
            handler: Handler::new(),
            metrics: Metrics::new(registry),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn remove_peer(&mut self, peer_id: PeerId) {
        warn!("Removing peer {peer_id}, total peers: {}", self.peers.len());

        self.peers.remove(&peer_id);
    }

    pub fn handle_failed_connection(&mut self, connection_id: ConnectionId) {
        if !self.is_enabled() {
            return;
        }

        if let Some(mut connection_data) = self.handler.remove_pending_connection(&connection_id) {
            if connection_data.retries() < self.config.dial_max_retries {
                // Retry dialing after a delay
                connection_data.inc_retries();

                let tx_dial = self.tx_dial.clone();
                tokio::spawn(async move {
                    sleep(connection_data.next_delay()).await;
                    tx_dial.send(connection_data).unwrap_or_else(|e| {
                        error!("Error sending dial request to channel: {e}");
                    });
                });
            } else {
                // No more trials left
                error!(
                    "Failed to dial peer at {0} after {1} trials",
                    connection_data.multiaddr(),
                    connection_data.retries(),
                );

                self.metrics.increment_failure();
                self.check_if_idle();
            }
        }
    }

    fn register_failed_request(&mut self, request_id: OutboundRequestId) {
        if !self.is_enabled() {
            return;
        }

        self.handler.remove_pending_request(&request_id);
        self.metrics.increment_failure();
    }

    fn should_dial(
        &self,
        swarm: &Swarm<impl SendResponse>,
        connection_data: &ConnectionData,
    ) -> bool {
        connection_data.peer_id().as_ref().map_or(true, |id| {
            // Is not itself (peer id)
            id != swarm.local_peer_id()
            // Is not already connected
            && !swarm.is_connected(id)
        })
            // Has not already dialed
            && !self.handler.has_already_dialed(connection_data)
            // Is not itself (multiaddr)
            && !swarm.listeners().any(|addr| *addr == connection_data.multiaddr())
    }

    pub fn dial_peer(
        &mut self,
        swarm: &mut Swarm<impl SendResponse>,
        connection_data: ConnectionData,
    ) {
        if !self.should_dial(swarm, &connection_data) {
            return;
        }

        let dial_opts = connection_data.build_dial_opts();
        let connection_id = dial_opts.connection_id();

        self.handler.register_dialed_peer(&connection_data);

        self.handler
            .register_pending_connection(connection_id, connection_data.clone());

        // Do not count retries as new interactions
        if connection_data.retries() == 0 {
            self.metrics.increment_dial();
        }

        info!(
            "Dialing peer at {}, retry #{}",
            connection_data.multiaddr(),
            connection_data.retries()
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

            self.handle_failed_connection(connection_id);
        }
    }

    pub fn handle_connection(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
    ) {
        if !self.is_enabled() {
            return;
        }

        self.handler
            .register_connection_type(peer_id, endpoint.into());

        self.handler.remove_pending_connection(&connection_id);

        // This call is necessary to record the peer id of a
        // bootstrap node (which was unknown before)
        self.handler.register_dialed_peer_id(peer_id);

        // This check is necessary to handle the case where two
        // nodes dial each other at the same time, which can lead
        // to a connection established (dialer) event for one node
        // after the connection established (listener) event on the
        // same node. Hence it is possible that the request for
        // peers was already sent before this event.
        if self.handler.has_already_requested(&peer_id) {
            self.check_if_idle();
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

                info.listen_addrs.first().map(|addr| {
                    remaining_bootstrap_nodes.retain(|x| x != addr);
                    (Some(*peer_id), addr.clone())
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
        // Ignore if discovery is disabled or the peer is already known or
        // the peer has already been requested
        if !self.is_enabled()
            || self.peers.contains_key(&peer_id)
            || self.handler.has_already_requested(&peer_id)
        {
            self.handler.remove_connection_type(&peer_id);
            self.check_if_idle();
            return;
        }

        // Only request peers from dialed peers
        if self.handler.remove_connection_type(&peer_id) == Some(ConnectionType::Dial) {
            if let Some(request_response) = behaviour {
                debug!(%peer_id, "Requesting peers from peer");

                let request_id = request_response.send_request(
                    &peer_id,
                    behaviour::Request::Peers(self.get_all_peers_except(peer_id)),
                );

                self.handler.register_requested_peer_id(peer_id);
                self.handler.register_pending_request(request_id);
            } else {
                // This should never happen
                error!(
                    "Discovery is enabled, but request-response behaviour is unavailable for peer {peer_id}"
                );
            }
        }

        self.peers.insert(peer_id, info);

        info!(
            "Discovered peer {peer_id}, total peers: {}",
            self.peers.len()
        );

        self.check_if_idle();
    }

    pub fn check_if_idle(&mut self) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let (is_idle, pending_connections_len, pending_requests_len) = self.handler.is_idle();

        if is_idle {
            self.metrics.register_idle(self.peers.len());

            return true;
        }

        info!(
            "Discovery in progress ({}ms), {} pending connections, {} pending requests",
            self.metrics.elapsed().as_millis(),
            pending_connections_len,
            pending_requests_len
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
                    debug!(count = peers.len(), peer_id = %peer, "Received peers");

                    self.handler.remove_pending_request(&request_id);

                    self.process_received_peers(swarm, peers);
                    self.check_if_idle();
                }
            },

            behaviour::Event::OutboundFailure {
                request_id,
                peer,
                error,
            } => {
                error!("Outbound request to {peer} failed: {error}");

                self.register_failed_request(request_id);
                self.check_if_idle();
            }

            _ => {}
        }
    }
}

// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tracing::{self as _, debug, error, info, trace};

use libp2p::{
    identify,
    request_response::{self, OutboundRequestId},
    swarm::{dial_opts::DialOpts, ConnectionId},
    Multiaddr, PeerId, Swarm,
};

mod behaviour;
pub use behaviour::*;

const DISCOVERY_PROTOCOL: &str = "/malachite-discover/v1beta1";

#[derive(Debug)]
pub struct Discovery {
    pub peers: HashMap<PeerId, identify::Info>,
    pub is_enabled: bool,
    is_done: bool,
    bootstrap_nodes: Vec<Multiaddr>,
    dialed_peer_ids: HashSet<PeerId>,
    dialed_multiaddrs: HashSet<Multiaddr>,
    pending_connections: HashSet<ConnectionId>,
    requested_peer_ids: HashSet<PeerId>,
    pending_requests: HashSet<OutboundRequestId>,
    /// Performance metrics
    total_interactions: usize,
    total_interactions_failed: usize,
    start_time: Instant,
    duration: Duration,
}

impl Discovery {
    pub fn new(enable_discovery: bool, bootstrap_nodes: Vec<Multiaddr>) -> Self {
        Discovery {
            peers: HashMap::new(),
            is_enabled: enable_discovery,
            is_done: false,
            bootstrap_nodes: bootstrap_nodes,
            dialed_peer_ids: HashSet::new(),
            dialed_multiaddrs: HashSet::new(),
            pending_connections: HashSet::new(),
            requested_peer_ids: HashSet::new(),
            pending_requests: HashSet::new(),
            total_interactions: 0,
            total_interactions_failed: 0,
            start_time: Instant::now(),
            duration: Duration::default(),
        }
    }

    pub fn add_pending_connection(
        &mut self,
        connection_id: ConnectionId,
        peer_id: Option<&PeerId>,
        multiaddr: Option<&Multiaddr>,
    ) {
        if self.is_enabled {
            if let Some(peer_id) = peer_id {
                self.dialed_peer_ids.insert(peer_id.clone());
            }
            if let Some(multiaddr) = multiaddr {
                self.dialed_multiaddrs.insert(multiaddr.clone());
            }
            self.pending_connections.insert(connection_id);
            self.total_interactions += 1;
        }
    }

    pub fn register_failed_connection(&mut self, connection_id: &ConnectionId) {
        if self.is_enabled {
            self.pending_connections.remove(connection_id);
            self.total_interactions_failed += 1;
        }
    }

    fn register_failed_request(&mut self, request_id: OutboundRequestId) {
        if self.is_enabled {
            self.pending_requests.remove(&request_id);
            self.total_interactions_failed += 1;
        }
    }

    pub fn handle_dialer_connection(&mut self, peer_id: &PeerId, connection_id: &ConnectionId) {
        if self.is_enabled {
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
        if self.is_enabled && !self.is_done && !self.peers.contains_key(&peer_id) {
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

        self.peers.insert(peer_id, info);
    }

    pub fn check_if_done(&mut self) -> bool {
        if self.is_enabled
            && !self.is_done
            && self.pending_connections.is_empty()
            && self.pending_requests.is_empty()
        {
            self.is_done = true;
            self.duration = self.start_time.elapsed();
            info!(
                "Discovery finished in {}ms, dialed {} peers, {} successful, {} failed",
                self.duration.as_millis(),
                self.total_interactions,
                self.total_interactions - self.total_interactions_failed,
                self.total_interactions_failed,
            );
            true
        } else {
            if !self.pending_connections.is_empty() || !self.pending_requests.is_empty() {
                info!(
                    "Discovery in progress, {} pending connections, {} pending requests",
                    self.pending_connections.len(),
                    self.pending_requests.len(),
                );
            }
            false
        }
    }

    fn process_received_peers(
        &mut self,
        swarm: &mut Swarm<impl SendResponse>,
        peers: HashSet<(Option<PeerId>, Multiaddr)>,
    ) {
        // TODO check upper bound on number of peers
        for (peer_id, listen_addr) in peers {
            if peer_id.as_ref().map_or(false, |id| {
                id == swarm.local_peer_id()
                    || swarm.is_connected(id)
                    || self.dialed_peer_ids.contains(id)
            }) || self.dialed_multiaddrs.contains(&listen_addr)
            {
                continue;
            }

            let create_dial_opts = |peer_id: Option<PeerId>, listen_addr: Multiaddr| {
                if let Some(peer_id) = peer_id {
                    DialOpts::peer_id(peer_id)
                        .addresses(vec![listen_addr])
                        .build()
                } else {
                    DialOpts::unknown_peer_id().address(listen_addr).build()
                }
            };

            let dial_opts = create_dial_opts(peer_id, listen_addr.clone());
            let connection_id = dial_opts.connection_id();

            self.add_pending_connection(connection_id, peer_id.as_ref(), Some(&listen_addr));

            if let Err(e) = swarm.dial(dial_opts) {
                if let Some(peer_id) = peer_id {
                    error!("Error dialing peer {peer_id}: {e}");
                } else {
                    error!("Error dialing peer {listen_addr}: {e}");
                }
                self.pending_connections.remove(&connection_id);
                self.total_interactions_failed += 1;
            }
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

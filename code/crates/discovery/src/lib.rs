use std::collections::HashMap;

use tracing::{debug, error, info, warn};

use malachitebft_metrics::Registry;

use libp2p::{identify, kad, request_response, swarm::ConnectionId, Multiaddr, PeerId, Swarm};

mod util;

mod behaviour;
pub use behaviour::*;

mod connection;
use connection::ConnectionData;

pub mod config;
pub use config::Config;

mod controller;
use controller::Controller;

mod handlers;
use handlers::selection::selector::Selector;

mod metrics;
use metrics::Metrics;

mod request;

#[derive(Debug, PartialEq)]
enum State {
    Bootstrapping,
    Extending(usize), // Target number of peers
    Idle,
}

// The usage of `OutboundConnection` is to keep track of the persistent connection status
// of a peer with its connection id. The connection id is an option as one can try to upgrade
// a connection that does not exist yet (hence, no connection id exists yet).
#[derive(Debug)]
struct OutboundConnection {
    connection_id: Option<ConnectionId>,
    is_persistent: bool,
}

#[derive(Debug)]
pub struct Discovery<C>
where
    C: DiscoveryClient,
{
    config: Config,
    state: State,

    selector: Box<dyn Selector<C>>,

    bootstrap_nodes: Vec<(Option<PeerId>, Multiaddr)>,
    discovered_peers: HashMap<PeerId, identify::Info>,
    active_connections: HashMap<PeerId, Vec<ConnectionId>>,
    outbound_connections: HashMap<PeerId, OutboundConnection>,
    inbound_connections: HashMap<PeerId, ConnectionId>,

    pub controller: Controller,
    metrics: Metrics,
}

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
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
            match config.bootstrap_protocol {
                config::BootstrapProtocol::Kademlia => {
                    debug!("Using Kademlia bootstrap");

                    State::Bootstrapping
                }

                config::BootstrapProtocol::Full => {
                    debug!("Using full bootstrap");

                    State::Extending(config.num_outbound_peers)
                }
            }
        } else {
            State::Idle
        };

        Self {
            config,
            state,

            selector: Discovery::get_selector(config.bootstrap_protocol, config.selector),

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

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn active_connections_len(&self) -> usize {
        self.active_connections.values().map(Vec::len).sum()
    }

    pub fn on_network_event(
        &mut self,
        swarm: &mut Swarm<C>,
        network_event: behaviour::NetworkEvent,
    ) {
        match network_event {
            behaviour::NetworkEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                result,
                step,
                ..
            }) => match result {
                kad::QueryResult::Bootstrap(Ok(_)) => {
                    if step.last && self.state == State::Bootstrapping {
                        debug!("Discovery bootstrap successful");

                        self.handle_successful_bootstrap(swarm);
                    }
                }

                kad::QueryResult::Bootstrap(Err(error)) => {
                    error!("Discovery bootstrap failed: {error}");

                    if self.state == State::Bootstrapping {
                        self.handle_failed_bootstrap();
                    }
                }

                _ => {}
            },

            behaviour::NetworkEvent::Kademlia(_) => {}

            behaviour::NetworkEvent::RequestResponse(event) => {
                match event {
                    request_response::Event::Message {
                        peer,
                        connection_id,
                        message:
                            request_response::Message::Request {
                                request, channel, ..
                            },
                    } => match request {
                        behaviour::Request::Peers(peers) => {
                            debug!(peer_id = %peer, %connection_id, "Received peers request from peer");

                            self.handle_peers_request(swarm, peer, channel, peers);
                        }

                        behaviour::Request::Connect() => {
                            debug!(peer_id = %peer, %connection_id, "Received connect request from peer");

                            self.handle_connect_request(swarm, channel, peer, connection_id);
                        }
                    },

                    request_response::Event::Message {
                        peer,
                        connection_id,
                        message:
                            request_response::Message::Response {
                                response,
                                request_id,
                                ..
                            },
                    } => match response {
                        behaviour::Response::Peers(peers) => {
                            debug!(peer_id = %peer, %connection_id, count = peers.len(), "Received peers response from peer");

                            self.handle_peers_response(swarm, request_id, peers);
                        }

                        behaviour::Response::Connect(accepted) => {
                            debug!(peer_id = %peer, %connection_id, accepted, "Received connect response from peer");

                            self.handle_connect_response(
                                swarm,
                                request_id,
                                peer,
                                connection_id,
                                accepted,
                            );
                        }
                    },

                    request_response::Event::OutboundFailure {
                        peer,
                        request_id,
                        connection_id: _,
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
}

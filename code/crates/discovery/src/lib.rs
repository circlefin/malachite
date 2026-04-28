use std::collections::{HashMap, HashSet};

use tracing::{debug, error, info, warn};

use malachitebft_metrics::Registry;

use libp2p::core::SignedEnvelope;
use libp2p::{identify, kad, request_response, swarm::ConnectionId, Multiaddr, PeerId, Swarm};

mod behaviour;
pub use behaviour::*;

mod dial;
use dial::DialData;

pub mod config;
pub use config::Config;

mod controller;
use controller::Controller;

mod handlers;
use handlers::selection::selector::Selector;

mod metrics;
use metrics::Metrics;

mod rate_limiter;
use rate_limiter::DiscoveryRateLimiter;

mod request;

pub mod util;

#[derive(Debug, PartialEq)]
enum State {
    Bootstrapping,
    Extending(usize), // Target number of peers
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// Outbound connection (we dialed the peer)
    Outbound,
    /// Inbound connection (the peer dialed us)
    Inbound,
}

impl ConnectionDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Outbound => "outbound",
            Self::Inbound => "inbound",
        }
    }
}

/// Information about an established connection
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub direction: ConnectionDirection,
    pub remote_addr: Multiaddr,
}

#[derive(Debug, PartialEq)]
enum OutboundState {
    Pending,
    Confirmed,
}

#[derive(Debug)]
pub struct Discovery<C>
where
    C: DiscoveryClient,
{
    config: Config,
    state: State,

    selector: Box<dyn Selector<C>>,

    bootstrap_nodes: Vec<(Option<PeerId>, Vec<Multiaddr>)>,
    discovered_peers: HashMap<PeerId, identify::Info>,
    /// Signed peer records received from peers (cryptographically verified)
    signed_peer_records: HashMap<PeerId, SignedEnvelope>,
    active_connections: HashMap<PeerId, Vec<ConnectionId>>,
    /// Track connection info (direction and remote address) per connection
    pub connections: HashMap<ConnectionId, ConnectionInfo>,
    outbound_peers: HashMap<PeerId, OutboundState>,
    inbound_peers: HashSet<PeerId>,

    /// Rate limiter for peers requests
    rate_limiter: DiscoveryRateLimiter,

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

        // Warn if discovery is enabled with persistent_peers_only
        if config.enabled && config.persistent_peers_only {
            warn!(
                "Discovery is enabled with persistent_peers_only mode. \
                 Discovered peers will be rejected unless they are in the persistent_peers list. \
                 Consider disabling discovery for a pure persistent-peers-only setup."
            );
        }

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

            selector: Discovery::get_selector(
                config.enabled,
                config.bootstrap_protocol,
                config.selector,
            ),

            bootstrap_nodes: bootstrap_nodes
                .clone()
                .into_iter()
                .map(|addr| (None, vec![addr]))
                .collect(),
            discovered_peers: HashMap::new(),
            signed_peer_records: HashMap::new(),
            active_connections: HashMap::new(),
            connections: HashMap::new(),
            outbound_peers: HashMap::new(),
            inbound_peers: HashSet::new(),

            rate_limiter: DiscoveryRateLimiter::default(),

            controller: Controller::new(),
            metrics: Metrics::new(registry, !config.enabled || bootstrap_nodes.is_empty()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if a peer connection is outbound
    pub fn is_outbound_peer(&self, peer_id: &PeerId) -> bool {
        self.outbound_peers.contains_key(peer_id)
    }

    /// Check if a peer connection is inbound
    pub fn is_inbound_peer(&self, peer_id: &PeerId) -> bool {
        self.inbound_peers.contains(peer_id)
    }

    /// Check if a peer is a persistent peer (in the bootstrap_nodes list)
    pub fn is_persistent_peer(&self, peer_id: &PeerId) -> bool {
        // XXX: The assumption here is bootstrap_nodes is a list of persistent peers.
        self.bootstrap_nodes
            .iter()
            .any(|(maybe_peer_id, _)| maybe_peer_id == &Some(*peer_id))
    }

    /// Returns an iterator over inbound peer IDs.
    pub fn inbound_peer_ids(&self) -> impl Iterator<Item = &PeerId> {
        self.inbound_peers.iter()
    }

    /// Returns true if there is room for additional inbound peers.
    pub fn has_inbound_capacity(&self) -> bool {
        self.inbound_peers.len() < self.config.num_inbound_peers
    }

    /// Returns true if the peer is ephemeral (connected but not categorized as inbound or outbound).
    pub fn is_ephemeral_peer(&self, peer_id: &PeerId) -> bool {
        self.active_connections.contains_key(peer_id)
            && !self.outbound_peers.contains_key(peer_id)
            && !self.inbound_peers.contains(peer_id)
    }

    /// Promote an ephemeral peer to inbound status.
    ///
    /// Fails if the peer is not ephemeral or if inbound capacity is full.
    /// The caller must evict an inbound peer first to free a slot.
    ///
    /// Any pending ephemeral close timer for this peer will be naturally
    /// cancelled by `should_close`, which checks inbound membership.
    ///
    /// Returns true if the peer was promoted.
    pub fn promote_to_inbound(&mut self, peer_id: PeerId) -> bool {
        if !self.is_ephemeral_peer(&peer_id) || !self.has_inbound_capacity() {
            return false;
        }
        self.inbound_peers.insert(peer_id);
        self.update_discovery_metrics();
        true
    }

    /// Evict an inbound peer by removing it from the inbound set and
    /// queuing its connections for immediate close.
    ///
    /// Returns true if the peer was evicted.
    pub fn evict_inbound_peer(&mut self, peer_id: PeerId) -> bool {
        if !self.inbound_peers.remove(&peer_id) {
            return false;
        }
        if let Some(connection_ids) = self.active_connections.get(&peer_id) {
            for connection_id in connection_ids.clone() {
                self.controller
                    .close
                    .add_to_queue((peer_id, connection_id), None);
            }
        }
        self.update_discovery_metrics();
        true
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
                kad::QueryResult::Bootstrap(Ok(_))
                    if step.last && self.state == State::Bootstrapping =>
                {
                    debug!("Discovery bootstrap successful");

                    self.handle_successful_bootstrap(swarm);
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
                        behaviour::Request::Peers(signed_records) => {
                            debug!(
                                peer_id = %peer, %connection_id,
                                count = signed_records.len(),
                                "Received peers request"
                            );

                            self.handle_peers_request(swarm, peer, channel, signed_records);
                        }

                        behaviour::Request::Connect() => {
                            debug!(peer_id = %peer, %connection_id, "Received connect request");

                            self.handle_connect_request(swarm, channel, peer);
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
                        behaviour::Response::Peers(signed_records) => {
                            debug!(
                                %peer, %connection_id,
                                count = signed_records.len(),
                                "Received peers response"
                            );

                            self.handle_peers_response(swarm, request_id, signed_records);
                        }

                        behaviour::Response::Connect(accepted) => {
                            debug!(%peer, %connection_id, accepted, "Received connect response");

                            self.handle_connect_response(swarm, request_id, peer, accepted);
                        }
                    },

                    request_response::Event::OutboundFailure {
                        peer,
                        request_id,
                        connection_id,
                        error,
                    } => {
                        error!(%peer, %connection_id, "Outbound request to failed: {error}");

                        if self.controller.peers_request.is_in_progress(&request_id) {
                            self.handle_failed_peers_request(swarm, request_id);
                        } else if self.controller.connect_request.is_in_progress(&request_id) {
                            self.handle_failed_connect_request(swarm, request_id);
                        } else {
                            // This should not happen
                            error!(%peer, %connection_id, "Unknown outbound request failure");
                        }
                    }

                    _ => {}
                }
            }
        }
    }

    /// Add a bootstrap node for persistent peer management
    pub fn add_bootstrap_node(&mut self, addr: Multiaddr) {
        // Check if this address already exists in bootstrap nodes
        if self
            .bootstrap_nodes
            .iter()
            .any(|(_, addrs)| addrs.contains(&addr))
        {
            info!("Bootstrap node already exists: {addr}");
            return;
        }

        // Extract peer_id from multiaddr if present
        let peer_id = addr.iter().find_map(|protocol| {
            if let libp2p::multiaddr::Protocol::P2p(peer_id) = protocol {
                Some(peer_id)
            } else {
                None
            }
        });

        // Add to bootstrap_nodes list
        self.bootstrap_nodes.push((peer_id, vec![addr]));

        info!(
            "Added bootstrap node, total: {}",
            self.bootstrap_nodes.len()
        );
    }

    /// Remove a bootstrap node for persistent peer management
    pub fn remove_bootstrap_node(&mut self, addr: &Multiaddr) -> bool {
        // Find matching bootstrap node by comparing addresses
        let pos = self
            .bootstrap_nodes
            .iter()
            .position(|(_, addrs)| addrs.iter().any(|a| a == addr));

        if let Some(index) = pos {
            self.bootstrap_nodes.remove(index);
            info!(
                "Removed bootstrap node, remaining: {}",
                self.bootstrap_nodes.len()
            );
            true
        } else {
            warn!("Bootstrap node not found for removal: {}", addr);
            false
        }
    }

    /// Get the peer_id associated with a bootstrap node address.
    ///
    /// This is useful when the peer_id is discovered when we successfully connect, via the TLS/noise handshake
    pub fn get_peer_id_for_addr(&self, addr: &Multiaddr) -> Option<PeerId> {
        self.bootstrap_nodes
            .iter()
            .find(|(_, addrs)| addrs.iter().any(|a| a == addr))
            .and_then(|(peer_id, _)| *peer_id)
    }

    /// Cancel any in-progress dial attempts for a given address and/or peer_id
    ///
    /// This is useful when removing a persistent peer to ensure we don't continue
    /// trying to dial them after they've been removed.
    pub fn cancel_dial_attempts(&mut self, addr: &Multiaddr, peer_id: Option<PeerId>) {
        use controller::PeerData;

        // Cancel dial attempts for the address
        let addr_without_p2p = util::strip_peer_id_from_multiaddr(addr);
        self.controller
            .dial
            .remove_done_on(&PeerData::Multiaddr(addr_without_p2p));

        // Cancel dial attempts for the peer_id if present
        if let Some(peer_id) = peer_id {
            self.controller
                .dial
                .remove_done_on(&PeerData::PeerId(peer_id));
        }
    }

    /// Test helper: simulate an active connection for a peer.
    #[cfg(feature = "test-utils")]
    pub fn add_test_active_connection(&mut self, peer_id: PeerId, connection_id: ConnectionId) {
        self.active_connections
            .entry(peer_id)
            .or_default()
            .push(connection_id);
    }

    /// Test helper: add a peer directly to the inbound set (bypasses capacity check).
    #[cfg(feature = "test-utils")]
    pub fn add_test_inbound_peer(&mut self, peer_id: PeerId) {
        self.inbound_peers.insert(peer_id);
    }
}

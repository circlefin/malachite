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
                .map(|addr| (util::peer_id_from_multiaddr(&addr), vec![addr]))
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

    /// Check if a peer id is pinned by the `/p2p/` component of a configured persistent
    /// peer address.
    fn is_pinned_persistent_peer(&self, peer_id: &PeerId) -> bool {
        self.bootstrap_nodes.iter().any(|(_, addrs)| {
            addrs
                .iter()
                .any(|addr| util::peer_id_from_multiaddr(addr) == Some(*peer_id))
        })
    }

    /// Check if a peer is a persistent peer (in the bootstrap_nodes list)
    pub fn is_persistent_peer(&self, peer_id: &PeerId) -> bool {
        self.bootstrap_nodes
            .iter()
            .any(|(maybe_peer_id, _)| maybe_peer_id == &Some(*peer_id))
            || self.is_pinned_persistent_peer(peer_id)
    }

    /// Whether this peer is allowed under `persistent_peers_only`.
    ///
    /// When the mode is off, every peer is allowed; when on, only peers on the
    /// persistent list. Shared by identify, connect-request, and dial gating.
    pub(crate) fn allows_peer_under_policy(&self, peer_id: &PeerId) -> bool {
        !self.config.persistent_peers_only || self.is_persistent_peer(peer_id)
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
                    } => {
                        self.handle_outbound_response(
                            swarm,
                            peer,
                            connection_id,
                            request_id,
                            response,
                        );
                    }

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

    /// Route an outbound discovery response by the pending-request map that owns
    /// `request_id`, then accept the payload only when its type matches.
    ///
    /// Peers takes priority when both maps claim the id, matching `OutboundFailure`.
    fn handle_outbound_response(
        &mut self,
        swarm: &mut Swarm<C>,
        peer: PeerId,
        connection_id: ConnectionId,
        request_id: request_response::OutboundRequestId,
        response: behaviour::Response,
    ) {
        if self.controller.peers_request.is_in_progress(&request_id) {
            match response {
                behaviour::Response::Peers(signed_records) => {
                    debug!(
                        %peer, %connection_id,
                        count = signed_records.len(),
                        "Received peers response"
                    );

                    self.handle_peers_response(swarm, request_id, signed_records);
                }
                behaviour::Response::Connect(_) => {
                    warn!(
                        %peer,
                        %connection_id,
                        %request_id,
                        "Received Connect response for pending Peers request; clearing peers request without retry"
                    );

                    self.metrics.increment_total_failed_peer_requests();
                    self.controller
                        .peers_request
                        .remove_in_progress(&request_id);
                    self.make_extension_step(swarm);
                }
            }
        } else if self.controller.connect_request.is_in_progress(&request_id) {
            match response {
                behaviour::Response::Connect(accepted) => {
                    debug!(%peer, %connection_id, accepted, "Received connect response");

                    self.handle_connect_response(swarm, request_id, peer, accepted);
                }
                behaviour::Response::Peers(_) => {
                    warn!(
                        %peer,
                        %connection_id,
                        %request_id,
                        "Received Peers response for pending Connect request; clearing connect request without retry"
                    );

                    self.metrics.increment_total_rejected_connect_requests();
                    if let Some(request_data) = self
                        .controller
                        .connect_request
                        .remove_in_progress(&request_id)
                    {
                        self.handle_connect_rejection(swarm, request_data.peer_id());
                    }
                }
            }
        } else {
            // This should not happen
            error!(%peer, %connection_id, %request_id, "Unknown outbound response");
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
        let peer_id = util::peer_id_from_multiaddr(&addr);

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
    ///
    /// Bootstrap dial history is keyed with the address as written (including a
    /// `/p2p/` component when the operator supplied one). Deleting a stripped
    /// copy misses that record and `dial_bootstrap_nodes` then skips the
    /// address forever after a remove-and-re-add.
    pub fn cancel_dial_attempts(&mut self, addr: &Multiaddr, peer_id: Option<PeerId>) {
        match peer_id {
            Some(peer_id) => {
                self.controller
                    .dial_clear_done_for_peer(peer_id, std::slice::from_ref(addr));
            }
            None => {
                self.controller
                    .dial
                    .remove_done_on(&controller::PeerData::Multiaddr(addr.clone()));
            }
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

    /// Test helper: replace the configured outbound candidate selector.
    #[cfg(test)]
    pub(crate) fn set_test_selector(&mut self, selector: Box<dyn Selector<C>>) {
        self.selector = selector;
    }
}

#[cfg(test)]
mod tests {
    use libp2p::swarm::dummy;
    use libp2p::{noise, tcp, yamux, SwarmBuilder};
    use malachitebft_metrics::Registry;

    use crate::dial::DialData;
    use crate::handlers::selection::selector::Selection;

    use super::*;

    fn discovery(bootstrap_nodes: Vec<Multiaddr>) -> Discovery<dummy::Behaviour> {
        let mut registry = Registry::default();
        Discovery::new(Config::new(false), bootstrap_nodes, &mut registry)
    }

    fn addr_with_peer_id(peer_id: &PeerId) -> Multiaddr {
        format!("/ip4/10.0.0.1/tcp/26656/p2p/{peer_id}")
            .parse()
            .expect("valid multiaddr")
    }

    /// A selector that always returns the same peers, whatever was discovered.
    #[derive(Debug)]
    struct FixedSelector(Vec<PeerId>);

    impl<C> Selector<C> for FixedSelector
    where
        C: DiscoveryClient,
    {
        fn try_select_n_outbound_candidates(
            &mut self,
            _swarm: &mut Swarm<C>,
            _discovered: &HashMap<PeerId, identify::Info>,
            _excluded: Vec<PeerId>,
            _n: usize,
        ) -> Selection<PeerId> {
            // Request count is the fixture length, or 1 when the fixture is empty.
            Selection::classify(self.0.clone(), self.0.len().max(1))
        }
    }

    fn build_swarm() -> Swarm<dummy::Behaviour> {
        SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .expect("tcp transport")
            .with_behaviour(|_| dummy::Behaviour)
            .expect("dummy behaviour")
            .build()
    }

    /// Identify info with no listen addresses, so the Kademlia routing-table update in
    /// `handle_new_peer` is skipped.
    fn identify_info() -> identify::Info {
        identify::Info {
            public_key: libp2p::identity::Keypair::generate_ed25519().public(),
            protocol_version: String::new(),
            agent_version: String::new(),
            listen_addrs: vec![],
            protocols: vec![],
            observed_addr: Multiaddr::empty(),
            signed_peer_record: None,
        }
    }

    #[test]
    fn discovery_off_does_not_count_an_inbound_peer_as_outbound() {
        let mut registry = Registry::default();
        let mut discovery: Discovery<dummy::Behaviour> =
            Discovery::new(Config::new(false), vec![], &mut registry);
        let mut swarm = build_swarm();
        let peer = PeerId::random();

        discovery.handle_new_peer(
            &mut swarm,
            ConnectionId::new_unchecked(1),
            peer,
            identify_info(),
        );
        assert!(discovery.is_inbound_peer(&peer));
        assert!(!discovery.is_outbound_peer(&peer));

        discovery
            .controller
            .dial
            .register_done_on(controller::PeerData::PeerId(peer));

        discovery.handle_new_peer(
            &mut swarm,
            ConnectionId::new_unchecked(2),
            peer,
            identify_info(),
        );

        assert!(
            discovery.is_inbound_peer(&peer),
            "the first inbound classification must be kept"
        );
        assert!(
            !discovery.is_outbound_peer(&peer),
            "a later dial must not also count the same peer as outbound"
        );
    }

    #[test]
    fn discovery_off_does_not_count_an_outbound_peer_as_inbound() {
        let mut registry = Registry::default();
        let mut discovery: Discovery<dummy::Behaviour> =
            Discovery::new(Config::new(false), vec![], &mut registry);
        let mut swarm = build_swarm();
        let peer = PeerId::random();

        discovery
            .controller
            .dial
            .register_done_on(controller::PeerData::PeerId(peer));

        discovery.handle_new_peer(
            &mut swarm,
            ConnectionId::new_unchecked(1),
            peer,
            identify_info(),
        );
        assert!(discovery.is_outbound_peer(&peer));
        assert!(!discovery.is_inbound_peer(&peer));

        discovery
            .controller
            .dial
            .remove_done_on(&controller::PeerData::PeerId(peer));

        discovery.handle_new_peer(
            &mut swarm,
            ConnectionId::new_unchecked(2),
            peer,
            identify_info(),
        );

        assert!(
            discovery.is_outbound_peer(&peer),
            "the first outbound classification must be kept"
        );
        assert!(
            !discovery.is_inbound_peer(&peer),
            "a later inbound connection must not also count the same peer as inbound"
        );
    }

    #[tokio::test]
    async fn no_outbound_candidate_starts_discovery_extension_on_repair() {
        // When the selector reports None (no candidate after exclusion), repair must
        // start a discovery extension rather than silently leaving the outbound set short.
        let mut config = Config::new(true);
        config.set_bootstrap_protocol(config::BootstrapProtocol::Full);
        config.set_selector(config::Selector::Random);
        config.set_peers_bounds(1, 1);

        let mut registry = Registry::default();
        let mut discovery: Discovery<dummy::Behaviour> =
            Discovery::new(config, vec![], &mut registry);
        // Empty bootstrap list finishes as Idle; leave outbound short so repair runs.
        assert_eq!(discovery.state, State::Idle);
        assert!(discovery.outbound_peers.is_empty());

        discovery.set_test_selector(Box::new(FixedSelector(vec![])));

        let known = PeerId::random();
        discovery.discovered_peers.insert(known, identify_info());

        let mut swarm = build_swarm();
        discovery.repair_outbound_peers(&mut swarm);

        assert_eq!(
            discovery.state,
            State::Extending(1),
            "Selection::None should start a discovery extension"
        );
        assert_eq!(
            discovery.controller.peers_request.queue_len(),
            1,
            "extension should queue a peers request against a known peer"
        );
    }

    #[tokio::test]
    async fn last_outbound_close_does_not_peers_request_disconnecting_peer() {
        // Regression for close/repair ordering: when the last outbound closes and
        // repair has no candidate, extension must not peers_request the peer we
        // are still cleaning up.
        let mut config = Config::new(true);
        config.set_bootstrap_protocol(config::BootstrapProtocol::Full);
        config.set_selector(config::Selector::Random);
        config.set_peers_bounds(1, 1);

        let mut registry = Registry::default();
        let mut discovery: Discovery<dummy::Behaviour> =
            Discovery::new(config, vec![], &mut registry);
        assert_eq!(discovery.state, State::Idle);

        discovery.set_test_selector(Box::new(FixedSelector(vec![])));

        let disconnecting = PeerId::random();
        let live = PeerId::random();
        let closed_cid = ConnectionId::new_unchecked(1);

        discovery
            .discovered_peers
            .insert(disconnecting, identify_info());
        discovery.discovered_peers.insert(live, identify_info());
        discovery
            .outbound_peers
            .insert(disconnecting, OutboundState::Confirmed);
        discovery
            .active_connections
            .insert(disconnecting, vec![closed_cid]);
        discovery.connections.insert(
            closed_cid,
            ConnectionInfo {
                direction: ConnectionDirection::Outbound,
                remote_addr: Multiaddr::empty(),
            },
        );

        let mut swarm = build_swarm();
        // Swarm is not connected to `disconnecting`, so this is treated as last close.
        discovery.handle_closed_connection(&mut swarm, disconnecting, closed_cid);

        assert!(
            !discovery.discovered_peers.contains_key(&disconnecting),
            "disconnecting peer should be cleaned up before repair/extension"
        );
        assert!(!discovery.outbound_peers.contains_key(&disconnecting));
        assert_eq!(discovery.state, State::Extending(1));
        assert_eq!(discovery.controller.peers_request.queue_len(), 1);

        let request = discovery
            .controller
            .peers_request
            .recv()
            .await
            .expect("extension should queue a peers request");
        assert_eq!(
            request.peer_id(),
            live,
            "extension must peers_request a still-known peer, not the disconnecting one"
        );
    }

    #[tokio::test]
    async fn persistent_peers_only_reject_frees_dial_slot_while_extending() {
        // A rejected identify must free dial.in_progress immediately. Queuing a close
        // while Extending previously deadlocked: can_close requires Idle, and Idle
        // waits for in_progress to empty.
        let mut config = Config::new(true);
        config.set_persistent_peers_only(true);
        config.set_bootstrap_protocol(config::BootstrapProtocol::Full);
        // The default `Kademlia` selector rejects the `Full` bootstrap protocol.
        config.set_selector(config::Selector::Random);
        config.set_peers_bounds(1, 1);

        let mut registry = Registry::default();
        let mut discovery: Discovery<dummy::Behaviour> =
            Discovery::new(config, vec![], &mut registry);
        discovery.state = State::Extending(1);

        let peer_id = PeerId::random();
        let connection_id = ConnectionId::new_unchecked(7);
        let addr: Multiaddr = "/ip4/10.0.0.1/tcp/26656".parse().unwrap();
        discovery
            .controller
            .dial
            .register_in_progress(connection_id, DialData::new(Some(peer_id), vec![addr]));
        assert!(!discovery.controller.dial.is_idle().0);

        let mut swarm = build_swarm();
        discovery.handle_new_peer(&mut swarm, connection_id, peer_id, identify_info());

        assert!(
            discovery.controller.dial.is_idle().0,
            "rejected peer must free its dial slot"
        );
        assert_eq!(
            discovery.state,
            State::Idle,
            "freeing the last dial must let extension finish"
        );
    }

    #[test]
    fn peer_only_entry_stays_persistent_after_disconnect_clears_the_runtime_slot() {
        let peer_id = PeerId::random();
        let peer_only: Multiaddr = format!("/p2p/{peer_id}").parse().expect("valid multiaddr");
        let mut discovery = discovery(vec![peer_only]);

        assert_eq!(discovery.bootstrap_nodes[0].0, Some(peer_id));
        assert!(discovery.is_persistent_peer(&peer_id));

        // cleanup_peer_on_disconnect clears the identify slot so an
        // address-only bootstrap can be re-identified. A peer-only entry
        // has nothing to match on reconnect.
        discovery.bootstrap_nodes[0].0 = None;

        assert!(
            discovery.is_persistent_peer(&peer_id),
            "a /p2p/<peer_id> persistent entry must still admit the peer after its last disconnect"
        );
    }

    #[test]
    fn cancel_dial_attempts_clears_the_written_bootstrap_address() {
        use crate::controller::PeerData;
        use crate::dial::DialData;

        let peer_id = PeerId::random();
        let addr = addr_with_peer_id(&peer_id);
        let mut discovery = discovery(vec![addr.clone()]);

        discovery.controller.dial_register_done_on(
            &DialData::new_bootstrap(Some(peer_id), vec![addr.clone()]),
            true,
        );
        assert!(
            discovery
                .controller
                .dial
                .is_done_on(&PeerData::Multiaddr(addr.clone())),
            "dial_bootstrap_nodes keys the record with /p2p/ still on"
        );
        assert!(discovery
            .controller
            .dial
            .is_done_on(&PeerData::PeerId(peer_id)));

        discovery.cancel_dial_attempts(&addr, Some(peer_id));

        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::Multiaddr(addr)),
            "remove-and-re-add must not skip this address as already attempted"
        );
        assert!(!discovery
            .controller
            .dial
            .is_done_on(&PeerData::PeerId(peer_id)));
    }

    #[test]
    fn cancel_dial_attempts_clears_written_address_without_peer_id() {
        use crate::controller::PeerData;
        use crate::dial::DialData;

        let peer_id = PeerId::random();
        let addr = addr_with_peer_id(&peer_id);
        let mut discovery = discovery(vec![addr.clone()]);

        discovery
            .controller
            .dial_register_done_on(&DialData::new_bootstrap(None, vec![addr.clone()]), true);
        assert!(discovery
            .controller
            .dial
            .is_done_on(&PeerData::Multiaddr(addr.clone())));

        discovery.cancel_dial_attempts(&addr, None);

        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::Multiaddr(addr)),
            "an unpinned persistent peer must still clear the as-written address"
        );
    }
}

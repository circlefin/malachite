use libp2p::{
    request_response::{OutboundRequestId, ResponseChannel},
    PeerId, Swarm,
};
use tracing::{debug, error, trace};

use crate::{
    behaviour::{self, Response},
    request::RequestData,
    Discovery, DiscoveryClient, OutboundState,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_connect_request(&self) -> bool {
        self.controller.connect_request.can_perform()
    }

    fn should_connect_request(&self, request_data: &RequestData) -> bool {
        // Has not already requested, or has requested but retries are allowed
        !self
            .controller
            .connect_request
            .is_done_on(&request_data.peer_id())
            || request_data.retry.count() != 0
    }

    pub fn connect_request_peer(&mut self, swarm: &mut Swarm<C>, request_data: RequestData) {
        if !self.should_connect_request(&request_data) {
            // done_on can leave a Pending outbound entry with no request in
            // flight (a prior refusal keeps the marker until disconnect).
            // Free the slot so repair can refill it; leave it alone if a request
            // for this peer is already in progress (duplicate queue item).
            let peer_id = request_data.peer_id();
            let has_in_flight = self
                .controller
                .connect_request
                .get_in_progress_iter()
                .any(|(_, data)| data.peer_id() == peer_id);
            if !has_in_flight && self.outbound_peers.get(&peer_id) == Some(&OutboundState::Pending)
            {
                self.outbound_peers.remove(&peer_id);
                if self.is_enabled() {
                    self.repair_outbound_peers(swarm);
                }
            }
            return;
        }

        self.controller
            .connect_request
            .register_done_on(request_data.peer_id());

        // Do not count retries as new interactions
        if request_data.retry.count() == 0 {
            self.metrics.increment_total_connect_requests();
        }

        debug!(
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

    /// Accept an inbound connect request if the peer already holds a slot or
    /// can be granted one. New slots go through [`Discovery::promote_to_inbound`],
    /// so Identify must have completed (`active_connections`).
    pub(crate) fn try_accept_inbound_connect(&mut self, peer: PeerId) -> bool {
        if !self.allows_peer_under_policy(&peer) {
            debug!("Rejecting upgrade of peer {peer} to inbound peer as it's non-persistent and persistent_peers_only mode is on");
            return false;
        }

        if self.is_outbound_peer(&peer) || self.is_inbound_peer(&peer) {
            debug!("Peer {peer} already holds a persistent slot");
            return true;
        }

        if self.promote_to_inbound(peer) {
            debug!("Upgrading peer {peer} to inbound peer");
            return true;
        }

        if !self.has_inbound_capacity() {
            debug!("Rejecting upgrade of peer {peer} to inbound peer as the limit is reached");
        } else {
            debug!("Rejecting upgrade of peer {peer} to inbound peer until identify completes");
        }
        false
    }

    pub(crate) fn handle_connect_request(
        &mut self,
        swarm: &mut Swarm<C>,
        channel: ResponseChannel<Response>,
        peer: PeerId,
    ) {
        let accepted = self.try_accept_inbound_connect(peer);

        if swarm
            .behaviour_mut()
            .send_response(channel, behaviour::Response::Connect(accepted))
            .is_err()
        {
            error!("Error sending connect response to {peer}");
        } else {
            trace!("Sent connect response to {peer}");
        }
    }

    pub(crate) fn handle_connect_response(
        &mut self,
        swarm: &mut Swarm<C>,
        request_id: OutboundRequestId,
        peer: PeerId,
        accepted: bool,
    ) {
        self.controller
            .connect_request
            .remove_in_progress(&request_id);

        if accepted {
            debug!("Successfully upgraded peer {peer} to outbound peer");

            if let Some(state) = self.outbound_peers.get_mut(&peer) {
                *state = OutboundState::Confirmed;
            }

            // if all outbound peers are persistent, discovery is done
            if self
                .outbound_peers
                .values()
                .all(|state| *state == OutboundState::Confirmed)
            {
                debug!("All outbound peers are persistent");

                self.metrics.initial_discovery_finished();
                self.update_discovery_metrics();
            }
        } else {
            debug!("Peer {peer} rejected connection upgrade to outbound peer");

            self.metrics.increment_total_rejected_connect_requests();

            self.handle_connect_rejection(swarm, peer);
        }
    }

    pub(crate) fn handle_connect_rejection(&mut self, swarm: &mut Swarm<C>, peer: PeerId) {
        self.outbound_peers.remove(&peer);

        // Leave done_on set so this peer is not connect-requested again while
        // still connected (cleared on disconnect). Repair can then refill the
        // slot from other candidates without reselecting this one.
        if self.is_enabled() {
            self.repair_outbound_peers(swarm);
        }
    }

    pub(crate) fn handle_failed_connect_request(
        &mut self,
        swarm: &mut Swarm<C>,
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

                self.handle_connect_rejection(swarm, request_data.peer_id());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use libp2p::identify;
    use libp2p::request_response::OutboundRequestId;
    use libp2p::swarm::dummy;
    use libp2p::swarm::ConnectionId;
    use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
    use malachitebft_metrics::Registry;

    use crate::config::{BootstrapProtocol, Config};
    use crate::handlers::selection::selector::{Selection, Selector};
    use crate::handlers::test_support;
    use crate::request::RequestData;
    use crate::{Discovery, DiscoveryClient, OutboundState};

    fn identified_peer(discovery: &mut Discovery<libp2p::swarm::dummy::Behaviour>, peer: PeerId) {
        discovery
            .active_connections
            .insert(peer, vec![ConnectionId::new_unchecked(1)]);
    }

    #[test]
    fn reject_unidentified_peer_with_inbound_capacity() {
        let mut discovery = test_support::discovery_with_inbound_capacity(2);
        let peer = PeerId::random();

        assert!(!discovery.try_accept_inbound_connect(peer));
        assert!(discovery.inbound_peers.is_empty());
    }

    #[test]
    fn accept_identified_peer_with_inbound_capacity() {
        let mut discovery = test_support::discovery_with_inbound_capacity(2);
        let peer = PeerId::random();
        identified_peer(&mut discovery, peer);

        assert!(discovery.try_accept_inbound_connect(peer));
        assert!(discovery.inbound_peers.contains(&peer));
    }

    #[test]
    fn accept_already_inbound_peer_without_reinsert() {
        let mut discovery = test_support::discovery_with_inbound_capacity(2);
        let peer = PeerId::random();
        discovery.inbound_peers.insert(peer);

        assert!(discovery.try_accept_inbound_connect(peer));
        assert_eq!(discovery.inbound_peers.len(), 1);
    }

    #[test]
    fn accept_already_outbound_peer_without_inbound_insert() {
        let mut discovery = test_support::discovery_with_inbound_capacity(2);
        let peer = PeerId::random();
        discovery
            .outbound_peers
            .insert(peer, OutboundState::Confirmed);

        assert!(discovery.try_accept_inbound_connect(peer));
        assert!(!discovery.inbound_peers.contains(&peer));
    }

    #[test]
    fn reject_non_persistent_peer_when_persistent_peers_only() {
        let mut config = Config::new(false);
        config.set_persistent_peers_only(true);
        config.set_peers_bounds(1, 2);
        let mut discovery = test_support::discovery(config);
        let peer = PeerId::random();
        identified_peer(&mut discovery, peer);

        assert!(!discovery.try_accept_inbound_connect(peer));
        assert!(discovery.inbound_peers.is_empty());
    }

    #[test]
    fn reject_then_accept_after_identify() {
        let mut discovery = test_support::discovery_with_inbound_capacity(2);
        let peer = PeerId::random();

        assert!(!discovery.try_accept_inbound_connect(peer));
        assert!(discovery.inbound_peers.is_empty());

        identified_peer(&mut discovery, peer);

        assert!(discovery.try_accept_inbound_connect(peer));
        assert!(discovery.inbound_peers.contains(&peer));
    }

    #[test]
    fn reject_identified_peer_when_inbound_at_capacity() {
        let mut discovery = test_support::discovery_with_inbound_capacity(1);
        let existing = PeerId::random();
        discovery.inbound_peers.insert(existing);

        let peer = PeerId::random();
        identified_peer(&mut discovery, peer);

        assert!(!discovery.try_accept_inbound_connect(peer));
        assert_eq!(discovery.inbound_peers.len(), 1);
        assert!(!discovery.inbound_peers.contains(&peer));
    }

    fn discovery_disabled() -> Discovery<dummy::Behaviour> {
        let mut registry = Registry::default();
        // Discovery disabled so rejection / skip paths do not run repair/extension.
        Discovery::new(Config::new(false), vec![], &mut registry)
    }

    fn discovery_enabled() -> Discovery<dummy::Behaviour> {
        let mut config = Config::new(true);
        config.set_bootstrap_protocol(BootstrapProtocol::Full);
        config.set_selector(crate::config::Selector::Random);
        config.set_peers_bounds(1, 1);
        let mut registry = Registry::default();
        Discovery::new(config, vec![], &mut registry)
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
            .with_swarm_config(|config| {
                config.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build()
    }

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

    /// Prefers peers in order, skipping any in `excluded` (same contract as production selectors).
    #[derive(Debug)]
    struct PreferPeers(Vec<PeerId>);

    impl<C> Selector<C> for PreferPeers
    where
        C: DiscoveryClient,
    {
        fn try_select_n_outbound_candidates(
            &mut self,
            _swarm: &mut Swarm<C>,
            _discovered: &HashMap<PeerId, identify::Info>,
            excluded: Vec<PeerId>,
            n: usize,
        ) -> Selection<PeerId> {
            if n == 0 {
                return Selection::None;
            }
            let chosen: Vec<PeerId> = self
                .0
                .iter()
                .filter(|peer_id| !excluded.contains(peer_id))
                .cloned()
                .take(n)
                .collect();
            match chosen.len() {
                0 => Selection::None,
                len if len < n => Selection::Only(chosen),
                _ => Selection::Exactly(chosen),
            }
        }
    }

    /// `OutboundRequestId` is a private-field `u64` newtype; production code only
    /// receives ids from `send_request`. Tests need a key for `in_progress`.
    fn fake_request_id(n: u64) -> OutboundRequestId {
        // SAFETY: `OutboundRequestId` is `pub struct OutboundRequestId(u64)`.
        unsafe { std::mem::transmute::<u64, OutboundRequestId>(n) }
    }

    #[tokio::test]
    async fn connect_rejection_keeps_done_on() {
        // A refused Connect frees the outbound slot but keeps done_on so the
        // same connected peer is not asked again.
        let mut discovery = discovery_disabled();
        let mut swarm = build_swarm();
        let peer = PeerId::random();

        discovery
            .outbound_peers
            .insert(peer, OutboundState::Pending);
        discovery.controller.connect_request.register_done_on(peer);

        discovery.handle_connect_rejection(&mut swarm, peer);

        assert!(
            !discovery.outbound_peers.contains_key(&peer),
            "rejected peer must leave outbound_peers"
        );
        assert!(
            discovery.controller.connect_request.is_done_on(&peer),
            "rejected peer must keep done_on until disconnect"
        );
    }

    #[tokio::test]
    async fn exhausted_connect_retries_keep_done_on() {
        // Exhausted retries share handle_connect_rejection with an explicit refusal.
        let mut discovery = discovery_disabled();
        let mut swarm = build_swarm();
        let peer = PeerId::random();
        let request_id = fake_request_id(7);

        discovery
            .outbound_peers
            .insert(peer, OutboundState::Pending);
        discovery.controller.connect_request.register_done_on(peer);
        discovery
            .controller
            .connect_request
            .register_in_progress(request_id, RequestData::new(peer));

        // Default connect_request_max_retries is 0, so the first failure terminates.
        discovery.handle_failed_connect_request(&mut swarm, request_id);

        assert!(!discovery.outbound_peers.contains_key(&peer));
        assert!(
            discovery.controller.connect_request.is_done_on(&peer),
            "exhausted retries must keep done_on like an explicit refusal"
        );
    }

    #[tokio::test]
    async fn rejection_repair_skips_rejected_peer_and_keeps_done_on() {
        // Prefer the rejected peer first. Repair must skip it because done_on
        // still excludes it, and the marker stays set.
        let mut discovery = discovery_enabled();
        let mut swarm = build_swarm();
        let rejected = PeerId::random();
        let refill = PeerId::random();

        discovery.discovered_peers.insert(rejected, identify_info());
        discovery.discovered_peers.insert(refill, identify_info());
        discovery.set_test_selector(Box::new(PreferPeers(vec![rejected, refill])));

        discovery
            .outbound_peers
            .insert(rejected, OutboundState::Pending);
        discovery
            .controller
            .connect_request
            .register_done_on(rejected);

        discovery.handle_connect_rejection(&mut swarm, rejected);

        assert!(
            !discovery.outbound_peers.contains_key(&rejected),
            "rejected peer must not be reselected in the same rejection stack"
        );
        assert_eq!(
            discovery.outbound_peers.get(&refill),
            Some(&OutboundState::Pending),
            "repair must refill with the next eligible candidate"
        );
        assert!(
            discovery.controller.connect_request.is_done_on(&rejected),
            "done_on must stay set so a later select cannot re-ask this peer"
        );
    }

    #[tokio::test]
    async fn skipped_connect_request_frees_pending_slot_without_in_flight() {
        // Stale done_on + Pending with nothing in flight: skip must free the slot.
        let mut discovery = discovery_disabled();
        let mut swarm = build_swarm();
        let peer = PeerId::random();

        discovery
            .outbound_peers
            .insert(peer, OutboundState::Pending);
        discovery.controller.connect_request.register_done_on(peer);

        discovery.connect_request_peer(&mut swarm, RequestData::new(peer));

        assert!(
            !discovery.outbound_peers.contains_key(&peer),
            "skipped Pending with no in-flight request must free the outbound slot"
        );
        assert!(
            discovery.controller.connect_request.is_done_on(&peer),
            "skip path must not clear done_on; the marker lasts until disconnect"
        );
    }

    #[tokio::test]
    async fn skipped_connect_request_repairs_with_another_candidate() {
        let mut discovery = discovery_enabled();
        let mut swarm = build_swarm();
        let stuck = PeerId::random();
        let refill = PeerId::random();

        discovery.discovered_peers.insert(stuck, identify_info());
        discovery.discovered_peers.insert(refill, identify_info());
        discovery.set_test_selector(Box::new(PreferPeers(vec![stuck, refill])));

        discovery
            .outbound_peers
            .insert(stuck, OutboundState::Pending);
        discovery.controller.connect_request.register_done_on(stuck);

        discovery.connect_request_peer(&mut swarm, RequestData::new(stuck));

        assert!(!discovery.outbound_peers.contains_key(&stuck));
        assert_eq!(
            discovery.outbound_peers.get(&refill),
            Some(&OutboundState::Pending),
            "skip path with discovery enabled must repair the freed slot"
        );
        assert!(
            discovery.controller.connect_request.is_done_on(&stuck),
            "skip keeps done_on so repair cannot reselect the stuck peer"
        );
    }

    #[tokio::test]
    async fn skipped_duplicate_keeps_pending_when_request_in_flight() {
        let mut discovery = discovery_disabled();
        let mut swarm = build_swarm();
        let peer = PeerId::random();
        let request_id = fake_request_id(11);

        discovery
            .outbound_peers
            .insert(peer, OutboundState::Pending);
        discovery.controller.connect_request.register_done_on(peer);
        discovery
            .controller
            .connect_request
            .register_in_progress(request_id, RequestData::new(peer));

        discovery.connect_request_peer(&mut swarm, RequestData::new(peer));

        assert_eq!(
            discovery.outbound_peers.get(&peer),
            Some(&OutboundState::Pending),
            "duplicate queue item must not drop a Pending that already has a request in flight"
        );
    }
}

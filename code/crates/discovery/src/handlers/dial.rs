use libp2p::{
    core::ConnectedPoint,
    swarm::{ConnectionId, DialError},
    Multiaddr, PeerId, Swarm,
};
use tracing::{debug, error, warn};

use crate::{
    controller::PeerData, dial::DialData, util::strip_peer_id_from_multiaddr, ConnectionDirection,
    ConnectionInfo, Discovery, DiscoveryClient,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_dial(&self) -> bool {
        self.controller.dial.can_perform()
    }

    fn should_dial(
        &self,
        swarm: &Swarm<C>,
        dial_data: &DialData,
        check_already_dialed: bool,
    ) -> bool {
        dial_data.peer_id().as_ref().is_none_or(|id| {
            // Is not itself (peer id)
            id != swarm.local_peer_id()
            // Is not already connected
            && !swarm.is_connected(id)
        })
            // Has not already dialed, or has dialed but retries are allowed
            && (!check_already_dialed || !self.controller.dial_is_done_on(dial_data) || dial_data.retry.count() != 0)
            // Is not itself (listen addresses)
            && !swarm.listeners().any(|addr| dial_data.listen_addrs().contains(addr))
            // Under persistent_peers_only, only dial the persistent list (or
            // bootstrap entries that *are* that list). Peer-exchange dials
            // enter here via add_to_dial_queue.
            && self.may_dial_under_policy(dial_data)
    }

    /// Whether `persistent_peers_only` allows this dial attempt.
    ///
    /// Bootstrap dials are always allowed — they come from the configured
    /// persistent list. Exchange dials need a peer id on that list.
    fn may_dial_under_policy(&self, dial_data: &DialData) -> bool {
        if dial_data.is_bootstrap() || !self.config.persistent_peers_only {
            return true;
        }
        dial_data
            .peer_id()
            .is_some_and(|id| self.allows_peer_under_policy(&id))
    }

    pub fn dial_peer(&mut self, swarm: &mut Swarm<C>, dial_data: DialData) {
        // Not checking if the peer was already dialed because it is done when
        // adding to the dial queue
        if !self.should_dial(swarm, &dial_data, false) {
            return;
        }

        let Some(dial_opts) = dial_data.build_dial_opts() else {
            warn!(
                "No addresses to dial for peer {:?}, skipping dial attempt",
                dial_data.peer_id()
            );
            return;
        };
        let connection_id = dial_opts.connection_id();

        // Register peer_id only, not addresses as they are untrusted
        self.controller.dial_register_done_on(&dial_data, false);

        self.controller
            .dial
            .register_in_progress(connection_id, dial_data.clone());

        // Do not count retries as new interactions
        if dial_data.retry.count() == 0 {
            self.metrics.increment_total_dials();
        }

        debug!(
            %connection_id,
            "Dialing peer {:?} at {:?}, retry #{}",
            dial_data.peer_id(),
            dial_data.listen_addrs(),
            dial_data.retry.count()
        );

        if let Err(e) = swarm.dial(dial_opts) {
            error!(
                %connection_id,
                "Error dialing peer {:?} at {:?}: {}",
                dial_data.peer_id(),
                dial_data.listen_addrs(),
                e
            );

            self.handle_failed_connection(swarm, connection_id, e);
        }
    }

    pub fn handle_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
    ) {
        match endpoint {
            d @ ConnectedPoint::Dialer { .. } => {
                let remote_addr = d.get_remote_address().clone();
                debug!(
                    peer = %peer_id, %connection_id, remote_address = %remote_addr,
                    "Connected to peer (outbound)"
                );

                // Track connection, direction and remote address
                self.connections.insert(
                    connection_id,
                    ConnectionInfo {
                        direction: ConnectionDirection::Outbound,
                        remote_addr,
                    },
                );

                // Only register as "done" for connections that the node initiated
                // This is needed in case the peer was dialed without knowing the peer id
                self.controller
                    .dial
                    .register_done_on(PeerData::PeerId(peer_id));
            }
            l @ ConnectedPoint::Listener { .. } => {
                let remote_addr = l.get_remote_address().clone();
                debug!(
                    peer = %peer_id, %connection_id, remote_address = %remote_addr,
                    "Accepted incoming connection from peer (inbound)"
                );

                // Track connection info: direction and remote address
                self.connections.insert(
                    connection_id,
                    ConnectionInfo {
                        direction: ConnectionDirection::Inbound,
                        remote_addr,
                    },
                );
            }
        }

        // This check is necessary to handle the case where two
        // nodes dial each other at the same time, which can lead
        // to a connection established (dialer) event for one node
        // after the connection established (listener) event on the
        // same node. Hence it is possible that the peer was already
        // added to the active connections.
        if self.active_connections.contains_key(&peer_id) {
            self.controller.dial.remove_in_progress(&connection_id);
            // Trigger potential extension step
            self.make_extension_step(swarm);
            return;
        }

        // Needed in case the peer was dialed without knowing the peer id
        self.controller
            .dial_add_peer_id_to_dial_data(connection_id, peer_id);
    }

    pub fn handle_failed_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        connection_id: ConnectionId,
        error: DialError,
    ) {
        if let Some(mut dial_data) = self.controller.dial.remove_in_progress(&connection_id) {
            // Skip retrying for errors that will occur again
            if matches!(
                error,
                DialError::LocalPeerId { .. }
                    | DialError::NoAddresses
                    | DialError::WrongPeerId { .. }
            ) {
                if let DialError::LocalPeerId { address } = &error {
                    if is_not_own_address(
                        address,
                        swarm.listeners().chain(swarm.external_addresses()),
                    ) {
                        warn!(
                            dialed_address = %address,
                            local_peer_id = %swarm.local_peer_id(),
                            "Dialed a peer that presented our own libp2p identity; another node is running with the same key"
                        );
                    }
                }

                self.make_extension_step(swarm);
                return;
            }

            if dial_data.retry.count() < self.config.dial_max_retries {
                // Retry dialing after a delay
                dial_data.retry.inc_count();

                let next_delay = dial_data.retry.next_delay();

                self.controller
                    .dial
                    .add_to_queue(dial_data.clone(), Some(next_delay));
            } else {
                // No more trials left
                error!(
                    "Failed to dial peer {:?} at {:?} after {} trials",
                    dial_data.peer_id(),
                    dial_data.listen_addrs(),
                    dial_data.retry.count(),
                );

                self.metrics.increment_total_failed_dials();

                // For bootstrap nodes, clear the done_on flag so they can be retried
                // by the periodic timer. We use the is_bootstrap flag set at creation time
                // rather than checking addresses, to prevent address spoofing attacks where
                // a malicious peer could advertise bootstrap addresses in peer exchange.
                if dial_data.is_bootstrap() {
                    // Clear done_on by address
                    for addr in dial_data.listen_addrs() {
                        self.controller
                            .dial
                            .remove_done_on(&crate::controller::PeerData::Multiaddr(addr));
                    }
                    debug!(
                        "Cleared dial history for bootstrap node addrs={:?} - will be retried by timer",
                        dial_data.listen_addrs()
                    );
                }

                self.make_extension_step(swarm);
            }
        }
    }

    pub(crate) fn add_to_dial_queue(&mut self, swarm: &Swarm<C>, dial_data: DialData) {
        if self.should_dial(swarm, &dial_data, true) {
            // Register peer_id only to avoid flooding the dial queue.
            // Don't register addresses because they may are untrusted (from peers response).
            self.controller.dial_register_done_on(&dial_data, false);

            self.controller.dial.add_to_queue(dial_data, None);
        }
    }

    pub fn dial_bootstrap_nodes(&mut self, swarm: &Swarm<C>) {
        for (peer_id, listen_addrs) in &self.bootstrap_nodes.clone() {
            // For bootstrap nodes, check if already attempted (done_on flag)
            // This prevents overlapping Fibonacci retry sequences since done_on is only cleared
            // after all retries are exhausted
            // The Fibonacci retry sequence is started when a connection fails, see handle_failed_connection()
            // We check by address since bootstrap nodes may not have peer_id
            let already_attempted = listen_addrs.iter().any(|addr| {
                self.controller
                    .dial
                    .is_done_on(&crate::controller::PeerData::Multiaddr(addr.clone()))
            });

            if already_attempted {
                continue;
            }

            // Skip identity-only entries (e.g. /p2p/<peer_id>) — they carry
            // no transport address to dial against.
            if listen_addrs.iter().all(crate::util::is_peer_id_only) {
                continue;
            }

            let dial_data = DialData::new_bootstrap(*peer_id, listen_addrs.clone());

            // For bootstrap nodes, always attempt to dial even if previously failed
            // This ensures persistent peers are retried indefinitely
            if self.should_dial(swarm, &dial_data, false) {
                debug!(
                    "Adding bootstrap node to dial queue: peer_id={:?}, queue_len_before={}, in_progress_len={}",
                    dial_data.peer_id(),
                    self.controller.dial.queue_len(),
                    self.controller.dial.is_idle().1
                );
                // For bootstrap nodes, register addresses too (trusted config)
                self.controller.dial_register_done_on(&dial_data, true);
                self.controller.dial.add_to_queue(dial_data, None);
            }
        }
    }
}

/// Whether `dialed` is none of the node's own addresses, ignoring any
/// `/p2p/<peer_id>` component.
fn is_not_own_address<'a>(
    dialed: &Multiaddr,
    mut own_addresses: impl Iterator<Item = &'a Multiaddr>,
) -> bool {
    let dialed = strip_peer_id_from_multiaddr(dialed);
    !own_addresses.any(|own| strip_peer_id_from_multiaddr(own) == dialed)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use libp2p::multiaddr::Protocol;
    use libp2p::swarm::dummy;
    use malachitebft_metrics::Registry;

    use crate::config::Config;

    use super::*;

    #[test]
    fn foreign_address_when_not_among_own() {
        let dialed = Multiaddr::from_str("/ip4/203.0.113.5/tcp/26656").unwrap();
        let own = [Multiaddr::from_str("/ip4/10.0.0.1/tcp/26656").unwrap()];

        assert!(is_not_own_address(&dialed, own.iter()));
    }

    #[test]
    fn own_address_not_foreign_ignoring_peer_id_suffix() {
        let own = Multiaddr::from_str("/ip4/10.0.0.1/tcp/26656").unwrap();
        let dialed = own.clone().with(Protocol::P2p(PeerId::random()));

        assert!(!is_not_own_address(&dialed, [own].iter()));
    }

    #[test]
    fn foreign_address_when_own_addresses_empty() {
        let dialed = Multiaddr::from_str("/ip4/10.0.0.1/tcp/26656").unwrap();

        assert!(is_not_own_address(&dialed, std::iter::empty()));
    }

    fn discovery(
        persistent_peers_only: bool,
        bootstrap: Vec<Multiaddr>,
    ) -> Discovery<dummy::Behaviour> {
        let mut config = Config::new(false);
        config.set_persistent_peers_only(persistent_peers_only);
        let mut registry = Registry::default();
        Discovery::new(config, bootstrap, &mut registry)
    }

    #[test]
    fn may_dial_rejects_exchanged_unknown_under_persistent_peers_only() {
        let discovery = discovery(true, vec![]);
        let dial = DialData::new(
            Some(PeerId::random()),
            vec!["/ip4/10.0.0.2/tcp/26656".parse().unwrap()],
        );
        assert!(!discovery.may_dial_under_policy(&dial));
    }

    #[test]
    fn may_dial_allows_exchanged_persistent_under_persistent_peers_only() {
        let peer_id = PeerId::random();
        let addr: Multiaddr = format!("/ip4/10.0.0.1/tcp/26656/p2p/{peer_id}")
            .parse()
            .unwrap();
        let discovery = discovery(true, vec![addr.clone()]);
        let dial = DialData::new(Some(peer_id), vec![addr]);
        assert!(discovery.may_dial_under_policy(&dial));
    }

    #[test]
    fn may_dial_allows_bootstrap_even_when_peer_id_unresolved() {
        let discovery = discovery(true, vec![]);
        let dial = DialData::new_bootstrap(None, vec!["/ip4/10.0.0.1/tcp/26656".parse().unwrap()]);
        assert!(discovery.may_dial_under_policy(&dial));
    }

    #[test]
    fn may_dial_allows_any_when_persistent_peers_only_off() {
        let discovery = discovery(false, vec![]);
        let dial = DialData::new(
            Some(PeerId::random()),
            vec!["/ip4/10.0.0.2/tcp/26656".parse().unwrap()],
        );
        assert!(discovery.may_dial_under_policy(&dial));
    }

    fn build_swarm() -> Swarm<dummy::Behaviour> {
        use libp2p::{noise, tcp, yamux, SwarmBuilder};
        use std::time::Duration;

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

    #[tokio::test]
    async fn add_to_dial_queue_skips_unknown_under_persistent_peers_only() {
        let mut discovery = discovery(true, vec![]);
        let swarm = build_swarm();
        let addr: Multiaddr = "/ip4/10.0.0.2/tcp/26656".parse().unwrap();

        assert_eq!(discovery.controller.dial.queue_len(), 0);
        discovery.add_to_dial_queue(&swarm, DialData::new(Some(PeerId::random()), vec![addr]));
        assert_eq!(
            discovery.controller.dial.queue_len(),
            0,
            "sender-supplied non-persistent peers must not consume dial queue slots"
        );
    }
}

use libp2p::{swarm::ConnectionId, Multiaddr, PeerId, Swarm};
use tracing::{debug, warn};

use crate::{
    util::{eq_ignore_peer_id, strip_peer_id_from_multiaddr},
    Discovery, DiscoveryClient, State,
};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_close(&mut self) -> bool {
        self.state == State::Idle && self.controller.close.can_perform()
    }

    /// Reject a connection that never became an accepted peer.
    ///
    /// Frees the dial slot and closes immediately. The close queue only drains
    /// in Idle, and Extending waits for `in_progress` to empty, so queuing a
    /// close here would deadlock discovery and leave the peer connected.
    pub(crate) fn reject_unaccepted_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        self.controller.dial.remove_in_progress(&connection_id);
        // Use the shared close helper for the should_close guard and debug log.
        // For a never-accepted peer this always closes.
        self.close_connection(swarm, peer_id, connection_id);
        self.make_extension_step(swarm);
    }

    fn should_close(&self, peer_id: PeerId, connection_id: ConnectionId) -> bool {
        // Only close ephemeral connections (i.e not inbound/outbound connections)
        // NOTE: a inbound or outbound connection can still be closed if it is not
        // part of the active connections to the peer. This is possible due to the
        // limit of the number of connections per peer.
        (!self.outbound_peers.contains_key(&peer_id) && !self.inbound_peers.contains(&peer_id))
            || self
                .active_connections
                .get(&peer_id)
                .is_none_or(|connection_ids| !connection_ids.contains(&connection_id))
    }

    pub fn close_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        if !self.should_close(peer_id, connection_id) {
            return;
        }

        debug!("Closing connection {connection_id} to peer {peer_id}");
        // Close the connection even if it is not active
        swarm.close_connection(connection_id);
    }

    pub fn handle_closed_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        let was_last_connection = !swarm.is_connected(&peer_id);
        let closed_remote_addr = self
            .connections
            .remove(&connection_id)
            .map(|c| c.remote_addr);

        let remove_active_peer =
            if let Some(connection_ids) = self.active_connections.get_mut(&peer_id) {
                if connection_ids.contains(&connection_id) {
                    warn!("Removing active connection {connection_id} to peer {peer_id}");
                    connection_ids.retain(|id| id != &connection_id);
                } else {
                    warn!("Non-established connection {connection_id} to peer {peer_id} closed");
                }

                connection_ids.is_empty()
            } else {
                false
            };

        if remove_active_peer {
            self.active_connections.remove(&peer_id);
        }

        // In case the connection was closed before identifying the peer
        self.controller.dial.remove_in_progress(&connection_id);

        let needs_outbound_repair = self.outbound_peers.contains_key(&peer_id);

        if needs_outbound_repair {
            warn!("Outbound connection {connection_id} to peer {peer_id} closed");

            if was_last_connection {
                warn!("Last connection to peer {peer_id} closed, removing from outbound peers");

                self.outbound_peers.remove(&peer_id);
            }
        } else if self.inbound_peers.contains(&peer_id) {
            warn!("Inbound connection {connection_id} to peer {peer_id} closed");

            if was_last_connection {
                warn!("Last connection to peer {peer_id} closed, removing from inbound peers");

                self.inbound_peers.remove(&peer_id);
            }
        }

        // Drop discovered-peer state before outbound repair, so a following
        // extension does not peers_request this peer.
        if was_last_connection {
            self.cleanup_peer_on_disconnect(peer_id, closed_remote_addr);
        }

        if needs_outbound_repair && self.is_enabled() {
            self.repair_outbound_peers(swarm);
        }

        self.update_discovery_metrics();
    }

    /// Always clear `dial.done_on` on the last close, including a connection
    /// that ends before Identify, so the next bootstrap tick can retry.
    fn cleanup_peer_on_disconnect(
        &mut self,
        peer_id: PeerId,
        closed_remote_addr: Option<Multiaddr>,
    ) {
        self.discovered_peers.remove(&peer_id);

        // Remove signed peer record (no longer connected, record may be stale)
        self.signed_peer_records.remove(&peer_id);

        // Clear rate limiter state for this peer
        self.rate_limiter.remove_peer(&peer_id);

        // Clear connect_request done_on to allow re-upgrading the peer on reconnection
        self.controller.connect_request.remove_done_on(&peer_id);
        // Clear peers_request done_on so a reconnecting peer can be
        // upgraded and asked for its peer list again.
        self.controller.peers_request.remove_done_on(&peer_id);

        // Only the closed remote address is trusted for bootstrap matching.
        // Identify listen addrs are self-reported and must not unlock a
        // configured bootstrap address.
        let mut addrs = Vec::new();
        if let Some(addr) = closed_remote_addr {
            push_addr_and_stripped(&mut addrs, addr);
        }

        let mut bootstrap_addrs = Vec::new();
        for (maybe_peer_id, listen_addrs) in self.bootstrap_nodes.iter_mut() {
            let matches_peer = *maybe_peer_id == Some(peer_id);
            let matches_addr = listen_addrs.iter().any(|bootstrap_addr| {
                addrs
                    .iter()
                    .any(|addr| eq_ignore_peer_id(addr, bootstrap_addr))
            });

            if !matches_peer && !matches_addr {
                continue;
            }

            // Reset an identified bootstrap slot so a restart that comes back
            // with a different peer id can be re-identified.
            if matches_peer {
                warn!(
                    "Resetting bootstrap node peer_id {} to allow re-identification",
                    peer_id
                );
                *maybe_peer_id = None;
            }

            bootstrap_addrs.extend(listen_addrs.iter().cloned());
        }
        addrs.extend(bootstrap_addrs);

        self.controller.dial_clear_done_for_peer(peer_id, &addrs);
    }
}

fn push_addr_and_stripped(addrs: &mut Vec<Multiaddr>, addr: Multiaddr) {
    let stripped = strip_peer_id_from_multiaddr(&addr);
    if stripped != addr {
        addrs.push(stripped);
    }
    addrs.push(addr);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::futures::StreamExt;
    use libp2p::multiaddr::Protocol;
    use libp2p::swarm::{dummy, ConnectionId, SwarmEvent};
    use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
    use malachitebft_metrics::Registry;

    use crate::controller::PeerData;
    use crate::dial::DialData;
    use crate::{config::Config, ConnectionDirection, ConnectionInfo, Discovery};

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

    async fn wait_listen_addr(swarm: &mut Swarm<dummy::Behaviour>) -> Multiaddr {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                return address;
            }
        }
    }

    #[tokio::test]
    async fn closing_identified_connection_keeps_peer_state_while_another_connection_remains() {
        let mut local_swarm = build_swarm();
        let mut remote_swarm = build_swarm();
        let remote_peer_id = *remote_swarm.local_peer_id();

        remote_swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
            .unwrap();
        let remote_addr = wait_listen_addr(&mut remote_swarm).await;
        local_swarm.dial(remote_addr.clone()).unwrap();

        let remaining_connection_id = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                tokio::select! {
                    event = local_swarm.select_next_some() => {
                        if let SwarmEvent::ConnectionEstablished { peer_id, connection_id, .. } = event {
                            assert_eq!(peer_id, remote_peer_id);
                            break connection_id;
                        }
                    }
                    _ = remote_swarm.select_next_some() => {}
                }
            }
        })
        .await
        .expect("timed out waiting for connection");

        let bootstrap_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26000".parse().unwrap();
        let mut registry = Registry::default();
        let mut discovery = Discovery::<dummy::Behaviour>::new(
            Config::new(false),
            vec![bootstrap_addr.clone()],
            &mut registry,
        );
        discovery.bootstrap_nodes[0].0 = Some(remote_peer_id);

        let closed_connection_id = ConnectionId::new_unchecked(usize::MAX);
        discovery
            .active_connections
            .insert(remote_peer_id, vec![closed_connection_id]);
        discovery.connections.insert(
            closed_connection_id,
            ConnectionInfo {
                direction: ConnectionDirection::Outbound,
                remote_addr: bootstrap_addr.clone(),
            },
        );
        discovery.connections.insert(
            remaining_connection_id,
            ConnectionInfo {
                direction: ConnectionDirection::Outbound,
                remote_addr,
            },
        );

        discovery.handle_closed_connection(&mut local_swarm, remote_peer_id, closed_connection_id);

        assert_eq!(
            discovery.get_peer_id_for_addr(&bootstrap_addr),
            Some(remote_peer_id)
        );
        assert!(!discovery.connections.contains_key(&closed_connection_id));
        assert!(discovery.connections.contains_key(&remaining_connection_id));
        assert!(!discovery.active_connections.contains_key(&remote_peer_id));

        assert!(local_swarm.close_connection(remaining_connection_id));
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                tokio::select! {
                    event = local_swarm.select_next_some() => {
                        if matches!(event, SwarmEvent::ConnectionClosed { connection_id, .. } if connection_id == remaining_connection_id) {
                            break;
                        }
                    }
                    _ = remote_swarm.select_next_some() => {}
                }
            }
        })
        .await
        .expect("timed out waiting for connection to close");

        discovery.handle_closed_connection(
            &mut local_swarm,
            remote_peer_id,
            remaining_connection_id,
        );

        assert_eq!(discovery.get_peer_id_for_addr(&bootstrap_addr), None);
        assert!(!discovery.connections.contains_key(&remaining_connection_id));
    }

    #[tokio::test]
    async fn last_connection_close_clears_peers_request_done_on() {
        let mut swarm = build_swarm();
        let peer_id = PeerId::random();
        let connection_id = ConnectionId::new_unchecked(1);
        let mut registry = Registry::default();
        let mut discovery = Discovery::<dummy::Behaviour>::new(
            Config::new(true),
            vec!["/ip4/127.0.0.1/tcp/26000".parse().unwrap()],
            &mut registry,
        );

        discovery.controller.peers_request.register_done_on(peer_id);
        discovery
            .controller
            .connect_request
            .register_done_on(peer_id);
        assert!(discovery.controller.peers_request.is_done_on(&peer_id));
        assert!(discovery.controller.connect_request.is_done_on(&peer_id));

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert!(!discovery.controller.peers_request.is_done_on(&peer_id));
        assert!(!discovery.controller.connect_request.is_done_on(&peer_id));
    }

    fn discovery_with_bootstrap(
        enabled: bool,
        bootstrap_addr: Multiaddr,
    ) -> Discovery<dummy::Behaviour> {
        let mut registry = Registry::default();
        Discovery::<dummy::Behaviour>::new(
            Config::new(enabled),
            vec![bootstrap_addr],
            &mut registry,
        )
    }

    fn mark_pre_identify_dial(
        discovery: &mut Discovery<dummy::Behaviour>,
        peer_id: PeerId,
        configured_addr: Multiaddr,
        remote_addr: Multiaddr,
    ) -> ConnectionId {
        mark_pre_identify_connection(
            discovery,
            peer_id,
            configured_addr,
            remote_addr,
            ConnectionDirection::Outbound,
        )
    }

    fn mark_pre_identify_connection(
        discovery: &mut Discovery<dummy::Behaviour>,
        peer_id: PeerId,
        configured_addr: Multiaddr,
        remote_addr: Multiaddr,
        direction: ConnectionDirection,
    ) -> ConnectionId {
        discovery
            .controller
            .dial_register_done_on(&DialData::new_bootstrap(None, vec![configured_addr]), true);
        discovery
            .controller
            .dial
            .register_done_on(PeerData::PeerId(peer_id));

        let connection_id = ConnectionId::new_unchecked(1);
        discovery.connections.insert(
            connection_id,
            ConnectionInfo {
                direction,
                remote_addr,
            },
        );
        connection_id
    }

    fn assert_dial_cleared(
        discovery: &Discovery<dummy::Behaviour>,
        peer_id: PeerId,
        configured_addr: &Multiaddr,
    ) {
        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::PeerId(peer_id)),
            "peer id must leave dial history so the next tick can retry"
        );
        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::Multiaddr(configured_addr.clone())),
            "bootstrap address must leave dial history so the next tick can retry"
        );
    }

    #[tokio::test]
    async fn last_close_before_identify_clears_address_only_bootstrap_dial_history() {
        let configured_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26000".parse().unwrap();
        let peer_id = PeerId::random();
        let remote_addr = configured_addr.clone().with(Protocol::P2p(peer_id));

        let mut swarm = build_swarm();
        let mut discovery = discovery_with_bootstrap(true, configured_addr.clone());
        let connection_id = mark_pre_identify_dial(
            &mut discovery,
            peer_id,
            configured_addr.clone(),
            remote_addr,
        );

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert_dial_cleared(&discovery, peer_id, &configured_addr);
        assert_eq!(discovery.get_peer_id_for_addr(&configured_addr), None);

        discovery.dial_bootstrap_nodes(&swarm);
        assert_eq!(
            discovery.controller.dial.queue_len(),
            1,
            "the next bootstrap tick must be allowed to queue the peer again"
        );
    }

    #[tokio::test]
    async fn last_close_before_identify_clears_dial_history_when_discovery_is_disabled() {
        let configured_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26001".parse().unwrap();
        let peer_id = PeerId::random();
        let remote_addr = configured_addr.clone().with(Protocol::P2p(peer_id));

        let mut swarm = build_swarm();
        let mut discovery = discovery_with_bootstrap(false, configured_addr.clone());
        let connection_id = mark_pre_identify_dial(
            &mut discovery,
            peer_id,
            configured_addr.clone(),
            remote_addr,
        );

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert_dial_cleared(&discovery, peer_id, &configured_addr);
    }

    #[tokio::test]
    async fn last_close_clears_discovered_peer_dial_history_when_discovery_is_enabled() {
        let mut swarm = build_swarm();
        let mut registry = Registry::default();
        let mut discovery =
            Discovery::<dummy::Behaviour>::new(Config::new(true), Vec::new(), &mut registry);
        let peer_id = PeerId::random();
        discovery
            .controller
            .dial
            .register_done_on(PeerData::PeerId(peer_id));

        let connection_id = ConnectionId::new_unchecked(1);
        discovery.connections.insert(
            connection_id,
            ConnectionInfo {
                direction: ConnectionDirection::Outbound,
                remote_addr: "/ip4/127.0.0.1/tcp/26002".parse().unwrap(),
            },
        );

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::PeerId(peer_id)),
            "a discovered peer must be dialable again after its last connection closes"
        );
    }

    #[tokio::test]
    async fn remaining_connection_keeps_pre_identify_dial_history() {
        let mut local_swarm = build_swarm();
        let mut remote_swarm = build_swarm();
        let remote_peer_id = *remote_swarm.local_peer_id();

        remote_swarm
            .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
            .unwrap();
        let remote_addr = wait_listen_addr(&mut remote_swarm).await;
        local_swarm.dial(remote_addr.clone()).unwrap();

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                tokio::select! {
                    event = local_swarm.select_next_some() => {
                        if matches!(event, SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == remote_peer_id) {
                            break;
                        }
                    }
                    _ = remote_swarm.select_next_some() => {}
                }
            }
        })
        .await
        .expect("timed out waiting for connection");

        let configured_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26003".parse().unwrap();
        let mut discovery = discovery_with_bootstrap(true, configured_addr.clone());
        let closed_connection_id = mark_pre_identify_dial(
            &mut discovery,
            remote_peer_id,
            configured_addr.clone(),
            configured_addr.clone().with(Protocol::P2p(remote_peer_id)),
        );

        discovery.handle_closed_connection(&mut local_swarm, remote_peer_id, closed_connection_id);

        assert!(
            discovery
                .controller
                .dial
                .is_done_on(&PeerData::PeerId(remote_peer_id)),
            "dial history must stay while another connection to the peer remains"
        );
        assert!(discovery
            .controller
            .dial
            .is_done_on(&PeerData::Multiaddr(configured_addr)));
    }

    #[tokio::test]
    async fn last_inbound_close_clears_peer_id_but_not_bootstrap_addr_with_ephemeral_source() {
        let configured_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26004".parse().unwrap();
        let inbound_remote: Multiaddr = "/ip4/127.0.0.1/tcp/54321".parse().unwrap();
        let peer_id = PeerId::random();

        let mut swarm = build_swarm();
        let mut discovery = discovery_with_bootstrap(true, configured_addr.clone());
        let connection_id = mark_pre_identify_connection(
            &mut discovery,
            peer_id,
            configured_addr.clone(),
            inbound_remote,
            ConnectionDirection::Inbound,
        );

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert!(
            !discovery
                .controller
                .dial
                .is_done_on(&PeerData::PeerId(peer_id)),
            "peer id must still be cleared on an inbound last close"
        );
        assert!(
            discovery
                .controller
                .dial
                .is_done_on(&PeerData::Multiaddr(configured_addr)),
            "an ephemeral inbound source port must not unlock the configured bootstrap listen address"
        );
    }

    #[tokio::test]
    async fn last_inbound_close_with_port_reuse_clears_bootstrap_addr() {
        let configured_addr: Multiaddr = "/ip4/127.0.0.1/tcp/26005".parse().unwrap();
        let peer_id = PeerId::random();

        let mut swarm = build_swarm();
        let mut discovery = discovery_with_bootstrap(true, configured_addr.clone());
        let connection_id = mark_pre_identify_connection(
            &mut discovery,
            peer_id,
            configured_addr.clone(),
            configured_addr.clone(),
            ConnectionDirection::Inbound,
        );

        discovery.handle_closed_connection(&mut swarm, peer_id, connection_id);

        assert_dial_cleared(&discovery, peer_id, &configured_addr);
    }
}

use libp2p::{core::ConnectedPoint, swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, error};

use crate::{connection::ConnectionData, controller::PeerData, Discovery, DiscoveryClient};

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
        connection_data: &ConnectionData,
        check_already_dialed: bool,
    ) -> bool {
        connection_data.peer_id().as_ref().is_none_or(|id| {
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

    pub fn dial_peer(&mut self, swarm: &mut Swarm<C>, connection_data: ConnectionData) {
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

        debug!(
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
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
    ) {
        match endpoint {
            ConnectedPoint::Dialer { .. } => {
                debug!(peer = %peer_id, %connection_id, "Connected to peer");
            }
            ConnectedPoint::Listener { .. } => {
                debug!(peer = %peer_id, %connection_id, "Accepted incoming connection from peer");
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
            // Trigger potential extension step
            self.make_extension_step(swarm);
            return;
        }

        // Needed in case the peer was dialed without knowing the peer id
        self.controller
            .dial_add_peer_id_to_connection_data(connection_id, peer_id);
    }

    pub fn handle_failed_connection(&mut self, swarm: &mut Swarm<C>, connection_id: ConnectionId) {
        if let Some(mut connection_data) = self.controller.dial.remove_in_progress(&connection_id) {
            if connection_data.retry.count() < self.config.dial_max_retries {
                // Retry dialing after a delay
                connection_data.retry.inc_count();

                let next_delay = connection_data.retry.next_delay();

                self.controller
                    .dial
                    .add_to_queue(connection_data.clone(), Some(next_delay));
            } else {
                // No more trials left
                error!(
                    "Failed to dial peer at {0} after {1} trials",
                    connection_data.multiaddr(),
                    connection_data.retry.count(),
                );

                self.metrics.increment_total_failed_dials();

                self.make_extension_step(swarm);
            }
        }
    }

    pub(crate) fn add_to_dial_queue(&mut self, swarm: &Swarm<C>, connection_data: ConnectionData) {
        if self.should_dial(swarm, &connection_data, true) {
            // Already register as dialed address to avoid flooding the dial queue
            // with the same dial attempts.
            self.controller.dial_register_done_on(&connection_data);

            self.controller.dial.add_to_queue(connection_data, None);
        }
    }

    pub fn dial_bootstrap_nodes(&mut self, swarm: &Swarm<C>) {
        for (peer_id, addr) in &self.bootstrap_nodes.clone() {
            self.add_to_dial_queue(swarm, ConnectionData::new(*peer_id, addr.clone()));
        }
    }
}

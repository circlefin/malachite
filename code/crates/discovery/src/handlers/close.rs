use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, error, warn};

use crate::{Discovery, DiscoveryClient, State};

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    pub fn can_close(&mut self) -> bool {
        self.state == State::Idle && self.controller.close.can_perform()
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
                .map_or(true, |connection_ids| {
                    !connection_ids.contains(&connection_id)
                })
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

        if self
            .active_connections
            .get(&peer_id)
            .is_some_and(|connections| connections.contains(&connection_id))
        {
            if swarm.close_connection(connection_id) {
                debug!("Closing connection {connection_id} to peer {peer_id}");
            } else {
                error!("Error closing connection {connection_id} to peer {peer_id}");
            }
        } else {
            warn!("Tried to close an unknown connection {connection_id} to peer {peer_id}");
        }
    }

    pub fn handle_closed_connection(
        &mut self,
        swarm: &mut Swarm<C>,
        peer_id: PeerId,
        connection_id: ConnectionId,
    ) {
        let mut was_last_connection = false;

        if let Some(connection_ids) = self.active_connections.get_mut(&peer_id) {
            if connection_ids.contains(&connection_id) {
                warn!("Removing active connection {connection_id} to peer {peer_id}");
                connection_ids.retain(|id| id != &connection_id);
                if connection_ids.is_empty() {
                    self.active_connections.remove(&peer_id);

                    was_last_connection = true;
                }
            } else {
                warn!("Non-established connection {connection_id} to peer {peer_id} closed");
            }
        }

        // In case the connection was closed before identifying the peer
        self.controller.dial.remove_in_progress(&connection_id);

        if self.outbound_peers.contains_key(&peer_id) {
            warn!("Outbound connection {connection_id} to peer {peer_id} closed");

            if was_last_connection {
                warn!("Last connection to peer {peer_id} closed, removing from outbound peers");

                self.outbound_peers.remove(&peer_id);
            }

            if self.is_enabled() {
                self.repair_outbound_peers(swarm);
            }
        } else if self.inbound_peers.contains(&peer_id) {
            warn!("Inbound connection {connection_id} to peer {peer_id} closed");

            if was_last_connection {
                warn!("Last connection to peer {peer_id} closed, removing from inbound peers");

                self.inbound_peers.remove(&peer_id);
            }
        }

        self.update_discovery_metrics();
    }
}

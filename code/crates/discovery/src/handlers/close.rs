use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use tracing::{error, info, warn};

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
        self.outbound_connections
            .get(&peer_id)
            .is_none_or(|out_conn| out_conn.connection_id != Some(connection_id))
            && self.inbound_connections.get(&peer_id) != Some(&connection_id)
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
                info!("Closing connection {connection_id} to peer {peer_id}");
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
        if let Some(connections) = self.active_connections.get_mut(&peer_id) {
            if connections.contains(&connection_id) {
                warn!("Removing active connection {connection_id} to peer {peer_id}");
                connections.retain(|id| id != &connection_id);
                if connections.is_empty() {
                    self.active_connections.remove(&peer_id);
                }
            } else {
                warn!("Non-established connection {connection_id} to peer {peer_id} closed");
            }
        }

        // In case the connection was closed before identifying the peer
        self.controller.dial.remove_in_progress(&connection_id);

        if self
            .outbound_connections
            .get(&peer_id)
            .is_some_and(|out_conn| out_conn.connection_id == Some(connection_id))
        {
            warn!("Outbound connection {connection_id} to peer {peer_id} closed");

            self.outbound_connections.remove(&peer_id);

            if self.is_enabled() {
                self.repair_outbound_connection(swarm);
            }
        } else if self.inbound_connections.get(&peer_id) == Some(&connection_id) {
            warn!("Inbound connection {connection_id} to peer {peer_id} closed");

            self.inbound_connections.remove(&peer_id);
        }

        self.update_connections_metrics();
    }
}

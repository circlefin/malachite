use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, warn};

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

        // Clean up discovered peers when all connections are closed
        if was_last_connection {
            self.cleanup_peer_on_disconnect(peer_id);
        }

        self.update_discovery_metrics();
    }

    /// Clean up peer state and dial history when the last connection to a peer is closed
    fn cleanup_peer_on_disconnect(&mut self, peer_id: PeerId) {
        let peer_info = self.discovered_peers.remove(&peer_id);

        // Find and reset the bootstrap node peer_id to allow re-identification
        // This handles the case where a bootstrap node restarts with a different peer_id
        for bootstrap_node in self.bootstrap_nodes.iter_mut() {
            if bootstrap_node.0 == Some(peer_id) {
                warn!(
                    "Resetting bootstrap node peer_id {} to allow re-identification",
                    peer_id
                );
                bootstrap_node.0 = None; // Reset to None so it can be re-identified
                self.controller
                    .dial_clear_done_for_peer(peer_id, &bootstrap_node.1);
                return;
            }
        }

        // Handle non-bootstrap peers when discovery is disabled
        if !self.is_enabled() {
            let addrs = peer_info.map(|info| info.listen_addrs).unwrap_or_default();
            self.controller.dial_clear_done_for_peer(peer_id, &addrs);
        }
    }
}

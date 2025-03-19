use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, warn};

use crate::{request::RequestData, Discovery, DiscoveryClient, OutboundConnection};

use super::selection::selector::Selection;

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn select_outbound_connections(&mut self, swarm: &mut Swarm<C>) {
        let n = self
            .config
            .num_outbound_peers
            .saturating_sub(self.outbound_connections.len());

        let peers = match self.selector.try_select_n_outbound_candidates(
            swarm,
            &self.discovered_peers,
            self.get_excluded_peers(),
            n,
        ) {
            Selection::Exactly(peers) => {
                debug!("Selected exactly {} outbound candidates", peers.len());
                peers
            }
            Selection::Only(peers) => {
                warn!("Selected only {} outbound candidates", peers.len());
                peers
            }
            Selection::None => {
                warn!("No outbound candidates available");
                return;
            }
        };

        for peer_id in peers {
            self.outbound_connections.insert(
                peer_id,
                OutboundConnection {
                    connection_id: None, // Will be set once the response is received
                    is_persistent: false,
                },
            );

            self.controller
                .connect_request
                .add_to_queue(RequestData::new(peer_id), None);
        }

        // Safety check: make sure that the inbound connections are not part of the outbound connections
        self.inbound_connections.retain(|peer_id, connection_id| {
            self.outbound_connections
                .get(peer_id)
                .is_none_or(|out_conn| out_conn.connection_id != Some(*connection_id))
        });
    }

    pub(crate) fn adjust_connections(&mut self, swarm: &mut Swarm<C>) {
        if !self.is_enabled() {
            return;
        }

        debug!("Adjusting connections");

        self.select_outbound_connections(swarm);

        let connections_to_close: Vec<(PeerId, ConnectionId)> = self
            .active_connections
            .iter()
            .flat_map(|(peer_id, connection_ids)| {
                connection_ids
                    .iter()
                    .map(|connection_id| (*peer_id, *connection_id))
            })
            // Remove inbound connections
            .filter(|(peer_id, connection_id)| {
                self.inbound_connections
                    .get(peer_id)
                    .is_none_or(|in_conn_id| *in_conn_id != *connection_id)
            })
            // Remove outbound connections
            .filter(|(peer_id, connection_id)| {
                self.outbound_connections
                    .get(peer_id)
                    .is_none_or(|out_conn| out_conn.connection_id != Some(*connection_id))
            })
            .collect();

        debug!(
            "Connections adjusted by disconnecting {} peers",
            connections_to_close.len(),
        );

        for (peer_id, connection_id) in connections_to_close {
            self.controller.close.add_to_queue(
                (peer_id, connection_id),
                Some(self.config.ephemeral_connection_timeout),
            );
        }
    }

    pub(crate) fn repair_outbound_connection(&mut self, swarm: &mut Swarm<C>) {
        if !self.is_enabled() || self.outbound_connections.len() >= self.config.num_outbound_peers {
            return;
        }

        debug!("Repairing an outbound connection");

        // Upgrade any inbound connection to outbound if any is available
        if let Some((peer_id, connection_id)) = self
            .inbound_connections
            .iter()
            // Do not select inbound connections whose peer id is already in the outbound connections
            // with another connection id
            .find(|(peer_id, _)| !self.outbound_connections.contains_key(peer_id))
            .map(|(peer_id, connection_id)| (*peer_id, *connection_id))
        {
            debug!("Upgrading connection {connection_id} of peer {peer_id} to outbound connection");

            self.inbound_connections.remove(&peer_id);
            self.outbound_connections.insert(
                peer_id,
                OutboundConnection {
                    connection_id: None, // Will be set once the response is received
                    is_persistent: true, // persistent connection already established
                },
            );

            // Consider the connect request as done
            self.controller.connect_request.register_done_on(peer_id);

            self.update_connections_metrics();

            return;
        }

        // If no inbound connection is available, then select a candidate
        match self.selector.try_select_n_outbound_candidates(
            swarm,
            &self.discovered_peers,
            self.get_excluded_peers(),
            1,
        ) {
            Selection::Exactly(peers) => {
                if let Some(peer_id) = peers.first() {
                    debug!("Trying to connect to peer {peer_id} to repair outbound connections");
                    self.outbound_connections.insert(
                        *peer_id,
                        OutboundConnection {
                            connection_id: None, // Will be set once the response is received
                            is_persistent: false,
                        },
                    );

                    self.controller
                        .connect_request
                        .add_to_queue(RequestData::new(*peer_id), None);
                }
            }
            _ => {
                // If no candidate is available, then trigger the discovery extension
                warn!("No available peers to repair outbound connections");

                self.initiate_extension_with_target(swarm, 1);
            }
        }
    }
}

use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use tracing::{debug, warn};

use crate::{request::RequestData, Discovery, DiscoveryClient};

use super::selection::selector::Selection;

impl<C> Discovery<C>
where
    C: DiscoveryClient,
{
    fn select_outbound_peers(&mut self, swarm: &mut Swarm<C>) {
        let n = self
            .config
            .num_outbound_peers
            .saturating_sub(self.outbound_peers.len());

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
            self.outbound_peers.insert(peer_id, false);

            self.controller
                .connect_request
                .add_to_queue(RequestData::new(peer_id), None);
        }

        // Safety check: make sure that the inbound connections are not part of the outbound connections
        self.inbound_peers
            .retain(|peer_id| !self.outbound_peers.contains_key(peer_id));
    }

    pub(crate) fn adjust_connections(&mut self, swarm: &mut Swarm<C>) {
        if !self.is_enabled() {
            return;
        }

        debug!("Adjusting connections");

        self.select_outbound_peers(swarm);

        let peers_to_disconnect: Vec<(PeerId, Vec<ConnectionId>)> = self
            .active_connections
            .iter()
            // Remove outbound peers
            .filter(|(peer_id, _)| !self.outbound_peers.contains_key(peer_id))
            // Remove inbound peers
            .filter(|(peer_id, _)| !self.inbound_peers.contains(peer_id))
            .map(|(peer_id, connection_ids)| ((*peer_id).clone(), connection_ids.clone()))
            .collect();

        debug!(
            "Connections adjusted by disconnecting {} peers",
            peers_to_disconnect.len()
        );

        for (peer_id, connection_ids) in peers_to_disconnect {
            for connection_id in connection_ids {
                self.controller.close.add_to_queue(
                    (peer_id, connection_id),
                    Some(self.config.ephemeral_connection_timeout),
                );
            }
        }
    }

    pub(crate) fn repair_outbound_peers(&mut self, swarm: &mut Swarm<C>) {
        if !self.is_enabled() || self.outbound_peers.len() >= self.config.num_outbound_peers {
            return;
        }

        debug!("Repairing outbound peers");

        // Upgrade any inbound peer to outbound if any is available
        if let Some(peer_id) = self
            .inbound_peers
            .iter()
            // Safety check: make sure that the inbound peer is not already an outbound peer
            .find(|peer_id| !self.outbound_peers.contains_key(peer_id))
            .cloned()
        {
            debug!("Upgrading peer {peer_id} to outbound peer");

            self.inbound_peers.remove(&peer_id);
            self.outbound_peers.insert(
                peer_id, true, // Already persistent
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
                    debug!("Trying to connect to peer {peer_id} to repair outbound peers");
                    self.outbound_peers.insert(*peer_id, false);

                    self.controller
                        .connect_request
                        .add_to_queue(RequestData::new(*peer_id), None);
                }
            }
            _ => {
                // If no candidate is available, then trigger the discovery extension
                warn!("No available peers to repair outbound peers");

                self.initiate_extension_with_target(swarm, 1);
            }
        }
    }
}

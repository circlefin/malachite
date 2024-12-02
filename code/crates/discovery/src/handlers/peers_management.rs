use libp2p::{swarm::ConnectionId, PeerId, Swarm};
use rand::seq::SliceRandom;
use tracing::{debug, info, warn};

use crate::{request::RequestData, Discovery, DiscoveryClient, OutboundConnection, State};

impl Discovery {
    fn get_next_peer_to_peers_request(
        &self,
        swarm: &mut Swarm<impl DiscoveryClient>,
    ) -> Option<PeerId> {
        let mut furthest_peer_id = None;

        // Find the furthest peer in the routing table that has not been requested.
        // Both iterators do not implement trait `DoubleEndedIterator`,
        // so we cannot use `rev()` to directly start from the furthest peer
        for kbucket in swarm.behaviour_mut().kbuckets() {
            for entry in kbucket.iter() {
                let peer_id = entry.node.key.preimage().clone();
                if !self.controller.peers_request.is_done_on(&peer_id) {
                    furthest_peer_id = Some(peer_id);
                }
            }
        }

        furthest_peer_id
    }

    fn select_n_outbound_candidates(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
        n: usize,
    ) -> Vec<PeerId> {
        debug!("Selecting {} outbound candidates", n);

        let mut candidates: Vec<PeerId> = Vec::new();

        // Select the candidates from the routing table first
        let routing_table_candidates: Vec<PeerId> = self
            .get_routing_table(swarm)
            .iter()
            .flat_map(|kbucket| kbucket.1.clone()) // get the peer ids
            // Remove already selected outbound connections
            .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
            // Remove peers to which a connect request has already been done
            .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
            .collect();

        debug!(
            "Routing table candidates: {}",
            routing_table_candidates.len()
        );

        candidates.extend(routing_table_candidates);

        if candidates.len() >= n {
            let mut rng = rand::thread_rng();

            return candidates.choose_multiple(&mut rng, n).cloned().collect();
        }

        // If we still need more candidates, select from the active connections
        let active_connections_candidates: Vec<PeerId> = self
            .active_connections
            .keys()
            // Remove already selected peers from the routing table
            .filter(|peer_id| !candidates.contains(peer_id))
            // Remove already selected outbound connections
            .filter(|peer_id| !self.outbound_connections.contains_key(peer_id))
            // Remove peers to which a connect request has already been done
            .filter(|peer_id| !self.controller.connect_request.is_done_on(peer_id))
            .cloned()
            .collect();

        debug!(
            "Active connections candidates: {}",
            active_connections_candidates.len(),
        );

        // Peers from the routing table are prioritized
        if active_connections_candidates.len() >= n - candidates.len() {
            let mut rng = rand::thread_rng();

            candidates.extend(
                active_connections_candidates.choose_multiple(&mut rng, n - candidates.len()),
            );
        } else {
            candidates.extend(active_connections_candidates);
        }

        if candidates.len() < n {
            warn!(
                "Not enough outbound candidates, expected {}, got {}",
                n,
                candidates.len()
            );
        }

        candidates
    }

    fn select_outbound_connections(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        let n = self
            .config
            .num_outbound_peers
            .saturating_sub(self.outbound_connections.len());

        for peer_id in self.select_n_outbound_candidates(swarm, n) {
            if let Some(connection_ids) = self.active_connections.get(&peer_id) {
                if connection_ids.len() > 1 {
                    warn!("Peer {peer_id} has more than one connection");
                    // TODO: refer to `OutboundConnection` struct TODO in lib.rs
                }
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: connection_ids.first().cloned(),
                        is_persistent: false,
                    },
                );
            } else {
                warn!("Peer {peer_id} has no active connection");
                self.outbound_connections.insert(
                    peer_id,
                    OutboundConnection {
                        connection_id: None,
                        is_persistent: false,
                    },
                );
            }

            self.controller
                .connect_request
                .add_to_queue(RequestData::new(peer_id), None);
        }

        // Safety check: make sure that the inbound connections are not part of the outbound connections
        self.inbound_connections.retain(|peer_id, connection_id| {
            self.outbound_connections
                .get(peer_id)
                .map_or(true, |out_conn| {
                    out_conn.connection_id != Some(*connection_id)
                })
        });
    }

    pub(crate) fn repair_outbound_connection(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() || self.outbound_connections.len() >= self.config.num_outbound_peers {
            return;
        }

        info!("Repairing an outbound connection");

        // Upgrade any inbound connection to outbound if any is available
        if let Some((peer_id, connection_id)) = self
            .inbound_connections
            .iter()
            // Do not select inbound connections whose peer id is already in the outbound connections
            // with another connection id
            .find(|(peer_id, _)| !self.outbound_connections.contains_key(peer_id))
            .map(|(peer_id, connection_id)| (*peer_id, *connection_id))
        {
            info!("Upgrading connection of peer {peer_id} to outbound connection");

            self.inbound_connections.remove(&peer_id);
            self.outbound_connections.insert(
                peer_id,
                OutboundConnection {
                    connection_id: Some(connection_id),
                    is_persistent: true, // persistent connection already established
                },
            );

            // Consider the connect request as done
            self.controller.connect_request.register_done_on(peer_id);

            self.update_connections_metrics(swarm);

            return;
        }

        // If no inbound connection is available, then select a candidate
        if let Some(peer_id) = self.select_n_outbound_candidates(swarm, 1).first() {
            info!("Trying to connect to peer {peer_id} to repair outbound connections");
            if let Some(connection_ids) = self.active_connections.get(peer_id) {
                if connection_ids.len() > 1 {
                    // TODO: refer to `OutboundConnection` struct TODO in lib.rs
                    warn!("Peer {peer_id} has more than one connection");
                }
                self.outbound_connections.insert(
                    *peer_id,
                    OutboundConnection {
                        connection_id: connection_ids.first().cloned(),
                        is_persistent: false,
                    },
                );
            } else {
                warn!("Peer {peer_id} has no active connection");
                self.outbound_connections.insert(
                    *peer_id,
                    OutboundConnection {
                        connection_id: None,
                        is_persistent: false,
                    },
                );
            }

            self.controller
                .connect_request
                .add_to_queue(RequestData::new(*peer_id), None);

            return;
        }

        // If no candidate is available, then trigger the discovery extension
        warn!("No available peers to repair outbound connections, triggering discovery extension");

        self.state = State::Extending;
        self.make_extension_step(swarm); // trigger extension
    }

    pub(crate) fn adjust_connections(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() {
            return;
        }

        info!("Adjusting connections");

        self.select_outbound_connections(swarm);

        let connections_to_close: Vec<(PeerId, ConnectionId)> = self
            .active_connections
            .iter()
            .flat_map(|(peer_id, connection_ids)| {
                connection_ids
                    .iter()
                    .map(|connection_id| (*peer_id, connection_id.clone()))
            })
            // Remove inbound connections
            .filter(|(peer_id, connection_id)| {
                self.inbound_connections
                    .get(peer_id)
                    .map_or(true, |in_conn_id| *in_conn_id != *connection_id)
            })
            // Remove outbound connections
            .filter(|(peer_id, connection_id)| {
                self.outbound_connections
                    .get(peer_id)
                    .map_or(true, |out_conn| {
                        out_conn.connection_id != Some(*connection_id)
                    })
            })
            .collect();

        info!(
            "Connections adjusted by disconnecting {} peers",
            connections_to_close.len(),
        );

        debug!(
            "Keeping outbound connections: {:?}, and inbound connections: {:?}",
            self.outbound_connections.keys(),
            self.inbound_connections.keys(),
        );

        for (peer_id, connection_id) in connections_to_close {
            self.controller.close.add_to_queue(
                (peer_id, connection_id),
                Some(self.config.ephemeral_connection_timeout),
            );
        }
    }

    pub(crate) fn make_extension_step(&mut self, swarm: &mut Swarm<impl DiscoveryClient>) {
        if !self.is_enabled() || self.state != State::Extending {
            return;
        }

        let (is_idle, pending_connections_len, pending_peers_requests_len) =
            self.controller.is_idle();
        let rx_dial_len = self.controller.dial.queue_len();
        let rx_peers_request_len = self.controller.peers_request.queue_len();

        if is_idle && rx_dial_len == 0 && rx_peers_request_len == 0 {
            // Done when we found enough peers to which we did not request persistent connection yet
            // to potentially upgrade them to outbound connections we are missing.
            if self
                .active_connections
                .iter()
                .filter(|(peer_id, _)| !self.controller.connect_request.is_done_on(peer_id))
                .count()
                < (self.config.num_outbound_peers - self.outbound_connections.len())
            {
                if let Some(peer_id) = self.get_next_peer_to_peers_request(swarm) {
                    info!(
                        "Discovery extension in progress ({}ms), requesting peers from peer {}",
                        self.metrics.elapsed().as_millis(),
                        peer_id
                    );

                    self.controller
                        .peers_request
                        .add_to_queue(RequestData::new(peer_id), None);

                    return;
                } else {
                    warn!("No more peers to request peers from");
                }
            }

            info!("Discovery extension done");
            info!(
                "Discovery found {} peers (expected {}) in {}ms",
                self.active_connections_len(),
                self.config.num_outbound_peers,
                self.metrics.elapsed().as_millis()
            );

            self.adjust_connections(swarm);

            self.state = State::Idle;
        } else {
            info!("Discovery extension in progress ({}ms), {} pending connections ({} in queue), {} pending requests ({} in queue)",
                self.metrics.elapsed().as_millis(),
                pending_connections_len,
                rx_dial_len,
                pending_peers_requests_len,
                rx_peers_request_len,
            );
        }
    }

    pub(crate) fn get_routing_table(
        &mut self,
        swarm: &mut Swarm<impl DiscoveryClient>,
    ) -> Vec<(u32, Vec<PeerId>)> {
        // To avoid any implementation error of calling this function when the discovery is disabled
        if !self.is_enabled() {
            return Vec::new();
        }

        let mut kbuckets: Vec<(u32, Vec<PeerId>)> = Vec::new();
        for kbucket in swarm.behaviour_mut().kbuckets() {
            let peers = kbucket
                .iter()
                .map(|entry| entry.node.key.preimage().clone())
                .collect();
            let index = kbucket.range().0.ilog2().unwrap_or(0);
            kbuckets.push((index, peers));
        }

        kbuckets
    }
}

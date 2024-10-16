use std::collections::{HashMap, HashSet};

use libp2p::{request_response::OutboundRequestId, swarm::ConnectionId, Multiaddr, PeerId};
use tracing::debug;

use crate::{ConnectionData, ConnectionType};

#[derive(Debug)]
pub struct Handler {
    dialed_peer_ids: HashSet<PeerId>,
    dialed_multiaddrs: HashSet<Multiaddr>,
    pending_connections: HashMap<ConnectionId, ConnectionData>,
    connections_types: HashMap<PeerId, ConnectionType>,
    requested_peer_ids: HashSet<PeerId>,
    pending_requests: HashSet<OutboundRequestId>,
}

impl Handler {
    pub fn new() -> Self {
        Handler {
            dialed_peer_ids: HashSet::new(),
            dialed_multiaddrs: HashSet::new(),
            pending_connections: HashMap::new(),
            connections_types: HashMap::new(),
            requested_peer_ids: HashSet::new(),
            pending_requests: HashSet::new(),
        }
    }

    pub fn register_dialed_peer_id(&mut self, peer_id: PeerId) {
        self.dialed_peer_ids.insert(peer_id);
    }

    pub fn register_dialed_peer(&mut self, connection_data: &ConnectionData) {
        if let Some(peer_id) = connection_data.peer_id() {
            self.dialed_peer_ids.insert(peer_id);
        }

        self.dialed_multiaddrs
            .insert(connection_data.multiaddr().clone());
    }

    pub fn has_already_dialed(&self, connection_data: &ConnectionData) -> bool {
        connection_data
            .peer_id()
            .as_ref()
            .map_or(false, |peer_id| self.dialed_peer_ids.contains(peer_id))
            || self
                .dialed_multiaddrs
                .contains(&connection_data.multiaddr())
    }

    pub fn register_pending_connection(
        &mut self,
        connection_id: ConnectionId,
        connection_data: ConnectionData,
    ) {
        self.pending_connections
            .insert(connection_id, connection_data);
    }

    pub fn remove_pending_connection(
        &mut self,
        connection_id: &ConnectionId,
    ) -> Option<ConnectionData> {
        self.pending_connections.remove(connection_id)
    }

    pub fn register_connection_type(&mut self, peer_id: PeerId, connection_type: ConnectionType) {
        match connection_type {
            ConnectionType::Dial => {
                debug!("Connected to {peer_id}");
            }
            ConnectionType::Listen => {
                debug!("Accepted incoming connection from {peer_id}");
            }
        }

        self.connections_types
            .entry(peer_id)
            .or_insert(connection_type);
    }

    pub fn remove_connection_type(&mut self, peer_id: &PeerId) -> Option<ConnectionType> {
        self.connections_types.remove(peer_id)
    }

    pub fn register_requested_peer_id(&mut self, peer_id: PeerId) {
        self.requested_peer_ids.insert(peer_id);
    }

    pub fn has_already_requested(&self, peer_id: &PeerId) -> bool {
        self.requested_peer_ids.contains(peer_id)
    }

    pub fn register_pending_request(&mut self, request_id: OutboundRequestId) {
        self.pending_requests.insert(request_id);
    }

    pub fn remove_pending_request(&mut self, request_id: &OutboundRequestId) {
        self.pending_requests.remove(request_id);
    }

    pub fn is_idle(&self) -> (bool, usize, usize) {
        let is_idle = self.pending_connections.is_empty() && self.pending_requests.is_empty();
        let pending_connections_len = self.pending_connections.len();
        let pending_requests_len = self.pending_requests.len();
        (is_idle, pending_connections_len, pending_requests_len)
    }
}

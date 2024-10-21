use std::collections::{HashMap, HashSet};

use libp2p::{request_response::OutboundRequestId, swarm::ConnectionId, Multiaddr, PeerId};
use tracing::debug;

use crate::{request::RequestData, ConnectionData, ConnectionType};

const DEFAULT_MAX_CONCURRENT_DIALS: usize = 2;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 2;

#[derive(Debug)]
pub struct Handler {
    dialed_peer_ids: HashSet<PeerId>,
    dialed_multiaddrs: HashSet<Multiaddr>,
    max_concurrent_dials: usize,
    pending_connections: HashMap<ConnectionId, ConnectionData>,
    connections_types: HashMap<PeerId, ConnectionType>,
    requested_peer_ids: HashSet<PeerId>,
    max_concurrent_requests: usize,
    pending_requests: HashMap<OutboundRequestId, RequestData>,
}

impl Handler {
    pub fn new() -> Self {
        Handler {
            dialed_peer_ids: HashSet::new(),
            dialed_multiaddrs: HashSet::new(),
            max_concurrent_dials: DEFAULT_MAX_CONCURRENT_DIALS,
            pending_connections: HashMap::new(),
            connections_types: HashMap::new(),
            requested_peer_ids: HashSet::new(),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            pending_requests: HashMap::new(),
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

    pub fn add_peer_id_to_connection_data(&mut self, connection_id: ConnectionId, peer_id: PeerId) {
        if let Some(connection_data) = self.pending_connections.get_mut(&connection_id) {
            connection_data.set_peer_id(peer_id);
        }
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

    pub fn can_dial(&self) -> bool {
        self.pending_connections.len() < self.max_concurrent_dials
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

    pub fn remove_matching_pending_connections(&mut self, peer_id: &PeerId) -> Vec<ConnectionData> {
        let matching_connection_ids = self
            .pending_connections
            .iter()
            .filter_map(|(connection_id, connection_data)| {
                if connection_data.peer_id() == Some(*peer_id) {
                    Some(*connection_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        matching_connection_ids
            .into_iter()
            .filter_map(|connection_id| self.pending_connections.remove(&connection_id))
            .collect()
    }

    pub fn register_connection_type(&mut self, peer_id: PeerId, connection_type: ConnectionType) {
        match connection_type {
            ConnectionType::Dial => {
                debug!(%peer_id, "Connected to peer");
                self.connections_types.insert(peer_id, connection_type);
            }
            ConnectionType::Listen => {
                debug!(%peer_id, "Accepted incoming connection from peer");
                // Only set the connection type if it's not already set to Dial
                self.connections_types
                    .entry(peer_id)
                    .or_insert(connection_type);
            }
        }
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

    pub fn can_request(&self) -> bool {
        self.pending_requests.len() < self.max_concurrent_requests
    }

    pub fn register_pending_request(
        &mut self,
        request_id: OutboundRequestId,
        request_data: RequestData,
    ) {
        self.pending_requests.insert(request_id, request_data);
    }

    pub fn remove_pending_request(
        &mut self,
        request_id: &OutboundRequestId,
    ) -> Option<RequestData> {
        self.pending_requests.remove(request_id)
    }

    pub fn is_idle(&self) -> (bool, usize, usize) {
        let is_idle = self.pending_connections.is_empty() && self.pending_requests.is_empty();
        let pending_connections_len = self.pending_connections.len();
        let pending_requests_len = self.pending_requests.len();
        (is_idle, pending_connections_len, pending_requests_len)
    }
}

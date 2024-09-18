// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tracing::{self as _, info};

use libp2p::{
    identify, request_response::OutboundRequestId, swarm::ConnectionId, Multiaddr, PeerId,
};

pub mod behaviour;

#[derive(Debug)]
pub struct Discovery {
    pub peers: HashMap<PeerId, identify::Info>,
    pub is_enabled: bool,
    pub is_done: bool,
    pub dialed_peer_ids: HashSet<PeerId>,
    pub dialed_multiaddrs: HashSet<Multiaddr>,
    pub pending_connections: HashSet<ConnectionId>,
    pub requested_peer_ids: HashSet<PeerId>,
    pub pending_requests: HashSet<OutboundRequestId>,
    /// Performance metrics
    pub total_interactions: usize,
    pub total_interactions_failed: usize,
    start_time: Instant,
    duration: Duration,
}

impl Discovery {
    pub fn new(enable_discovery: bool) -> Self {
        Discovery {
            peers: HashMap::new(),
            is_enabled: enable_discovery,
            is_done: false,
            dialed_peer_ids: HashSet::new(),
            dialed_multiaddrs: HashSet::new(),
            pending_connections: HashSet::new(),
            requested_peer_ids: HashSet::new(),
            pending_requests: HashSet::new(),
            total_interactions: 0,
            total_interactions_failed: 0,
            start_time: Instant::now(),
            duration: Duration::default(),
        }
    }

    pub fn is_done(&mut self) {
        if self.is_enabled
            && !self.is_done
            && self.pending_connections.is_empty()
            && self.pending_requests.is_empty()
        {
            self.is_done = true;
            self.duration = self.start_time.elapsed();
            info!(
                "Discovery finished in {}ms, dialed {} peers, {} successful, {} failed",
                self.duration.as_millis(),
                self.total_interactions,
                self.total_interactions - self.total_interactions_failed,
                self.total_interactions_failed,
            );
        } else {
            info!(
                "Discovery in progress, {} pending connections, {} pending requests",
                self.pending_connections.len(),
                self.pending_requests.len(),
            );
        }
    }
}

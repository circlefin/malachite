use std::time::Duration;

use libp2p::PeerId;

use crate::util::FibonacciBackoff;

#[derive(Debug, Clone)]
pub struct RequestData {
    peer_id: PeerId,
    retries: usize,
    backoff: FibonacciBackoff,
}

impl RequestData {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            retries: 0,
            backoff: FibonacciBackoff::new(),
        }
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn retries(&self) -> usize {
        self.retries
    }

    pub fn inc_retries(&mut self) {
        self.retries += 1;
    }

    pub fn next_delay(&mut self) -> Duration {
        self.backoff
            .next()
            .expect("FibonacciBackoff is an infinite iterator")
    }
}

use std::time::Duration;

use libp2p::{swarm::dial_opts::DialOpts, Multiaddr, PeerId};

use crate::util::FibonacciBackoff;

#[derive(Debug, Clone)]
pub struct ConnectionData {
    peer_id: Option<PeerId>,
    multiaddr: Multiaddr,
    retries: usize,
    backoff: FibonacciBackoff,
}

impl ConnectionData {
    pub fn new(peer_id: Option<PeerId>, multiaddr: Multiaddr) -> Self {
        Self {
            peer_id,
            multiaddr,
            retries: 0,
            backoff: FibonacciBackoff::new(),
        }
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id);
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        self.peer_id
    }

    pub fn multiaddr(&self) -> Multiaddr {
        self.multiaddr.clone()
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

    pub fn build_dial_opts(&self) -> DialOpts {
        if let Some(peer_id) = self.peer_id {
            DialOpts::peer_id(peer_id)
                .addresses(vec![self.multiaddr.clone()])
                .build()
        } else {
            DialOpts::unknown_peer_id()
                .address(self.multiaddr.clone())
                .build()
        }
    }
}

use std::time::Duration;

use libp2p::{core::ConnectedPoint, swarm::dial_opts::DialOpts, Multiaddr, PeerId};

pub(crate) type Trial = usize;
pub(crate) const DIAL_MAX_TRIALS: Trial = 5;

#[derive(Debug, Clone)]
pub(crate) struct FibonacciDelay {
    current: Trial,
    next: Trial,
}

impl FibonacciDelay {
    pub(crate) fn new() -> Self {
        // Start from 1 second
        FibonacciDelay {
            current: 1,
            next: 1,
        }
    }
}

impl Iterator for FibonacciDelay {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        let new_next = self.current + self.next;
        self.current = self.next;
        self.next = new_next;
        Some(Duration::from_secs(self.current as u64))
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionData {
    peer_id: Option<PeerId>,
    multiaddr: Multiaddr,
    trial: Trial,
    fib_delay: FibonacciDelay,
}

impl ConnectionData {
    pub fn new(peer_id: Option<PeerId>, multiaddr: Multiaddr) -> Self {
        ConnectionData {
            peer_id,
            multiaddr,
            trial: 1,
            fib_delay: FibonacciDelay::new(),
        }
    }

    pub(crate) fn get_peer_id(&self) -> Option<PeerId> {
        self.peer_id.clone()
    }

    pub(crate) fn get_multiaddr(&self) -> Multiaddr {
        self.multiaddr.clone()
    }

    pub(crate) fn get_trial(&self) -> Trial {
        self.trial
    }

    pub(crate) fn increment_trial(&mut self) {
        self.trial += 1;
    }

    pub(crate) fn next_delay(&mut self) -> Duration {
        self.fib_delay.next().unwrap()
    }

    pub(crate) fn build_dial_opts(&self) -> DialOpts {
        if let Some(peer_id) = self.peer_id.clone() {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConnectionType {
    Dial,   // The node initiated the connection
    Listen, // The node received the connection
}

impl From<ConnectedPoint> for ConnectionType {
    fn from(connected_point: ConnectedPoint) -> Self {
        match connected_point {
            ConnectedPoint::Dialer { .. } => ConnectionType::Dial,
            ConnectedPoint::Listener { .. } => ConnectionType::Listen,
        }
    }
}

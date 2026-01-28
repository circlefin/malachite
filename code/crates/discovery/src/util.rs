use std::time::Duration;

use libp2p::Multiaddr;

/// Strip /p2p/<peer_id> component from a Multiaddr for address comparison.
/// This allows comparing addresses regardless of whether they include a peer ID.
pub fn strip_peer_id_from_multiaddr(addr: &Multiaddr) -> Multiaddr {
    use libp2p::multiaddr::Protocol;

    let mut result = Multiaddr::empty();
    for protocol in addr.iter() {
        if !matches!(protocol, Protocol::P2p(_)) {
            result.push(protocol);
        }
    }
    result
}

#[derive(Debug, Clone)]
struct FibonacciBackoff {
    current: u64,
    next: u64,
}

impl FibonacciBackoff {
    pub fn new() -> Self {
        // Start from 1 second
        Self {
            current: 1000,
            next: 1000,
        }
    }
}

impl Iterator for FibonacciBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        let new_next = self.current + self.next;
        self.current = self.next;
        self.next = new_next;

        Some(Duration::from_millis(self.current))
    }
}

#[derive(Debug, Clone)]
pub struct Retry {
    count: usize,
    backoff: FibonacciBackoff,
}

impl Retry {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            count: 0,
            backoff: FibonacciBackoff::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn inc_count(&mut self) {
        self.count += 1;
    }

    pub fn next_delay(&mut self) -> Duration {
        self.backoff
            .next()
            .expect("FibonacciBackoff is an infinite iterator")
    }
}

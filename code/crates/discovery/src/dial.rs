use libp2p::{swarm::dial_opts::DialOpts, Multiaddr, PeerId};

use crate::util::Retry;

#[derive(Debug, Clone)]
pub struct DialData {
    peer_id: Option<PeerId>,
    listen_addrs: Vec<Multiaddr>,
    pub retry: Retry,
    /// Whether this dial originated from bootstrap/persistent peer configuration.
    /// Used to determine retry behavior, only bootstrap dials get unlimited retries.
    is_bootstrap: bool,
}

impl DialData {
    pub fn new(peer_id: Option<PeerId>, listen_addrs: Vec<Multiaddr>) -> Self {
        Self {
            peer_id,
            listen_addrs,
            retry: Retry::new(),
            is_bootstrap: false,
        }
    }

    /// Create a DialData for a bootstrap/persistent peer
    pub fn new_bootstrap(peer_id: Option<PeerId>, listen_addrs: Vec<Multiaddr>) -> Self {
        Self {
            peer_id,
            listen_addrs,
            retry: Retry::new(),
            is_bootstrap: true,
        }
    }

    /// Returns true if this dial originated from bootstrap/persistent peer configuration
    pub fn is_bootstrap(&self) -> bool {
        self.is_bootstrap
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id);
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        self.peer_id
    }

    pub fn listen_addrs(&self) -> Vec<Multiaddr> {
        self.listen_addrs.clone()
    }

    pub fn build_dial_opts(&self) -> Option<DialOpts> {
        if let Some(addr) = self.listen_addrs.first() {
            if let Some(peer_id) = self.peer_id {
                Some(
                    DialOpts::peer_id(peer_id)
                        .addresses(self.listen_addrs.clone())
                        .allocate_new_port()
                        .build(),
                )
            } else {
                Some(
                    DialOpts::unknown_peer_id()
                        .address(addr.clone())
                        .allocate_new_port()
                        .build(),
                )
            }
        } else {
            None // No addresses to dial
        }
    }
}

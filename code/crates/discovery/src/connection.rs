use libp2p::{swarm::dial_opts::DialOpts, Multiaddr, PeerId};

use crate::util::Retry;

#[derive(Debug, Clone)]
pub struct ConnectionData {
    peer_id: Option<PeerId>,
    multiaddr: Multiaddr,
    pub retry: Retry,
}

impl ConnectionData {
    pub fn new(peer_id: Option<PeerId>, multiaddr: Multiaddr) -> Self {
        Self {
            peer_id,
            multiaddr,
            retry: Retry::new(),
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

    pub fn build_dial_opts(&self) -> DialOpts {
        if let Some(peer_id) = self.peer_id {
            DialOpts::peer_id(peer_id)
                .addresses(vec![self.multiaddr.clone()])
                .allocate_new_port()
                .build()
        } else {
            DialOpts::unknown_peer_id()
                .address(self.multiaddr.clone())
                .allocate_new_port()
                .build()
        }
    }
}

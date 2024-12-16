use libp2p::PeerId;

use crate::util::Retry;

#[derive(Debug, Clone)]
pub struct RequestData {
    peer_id: PeerId,
    pub retry: Retry,
}

impl RequestData {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            retry: Retry::new(),
        }
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }
}

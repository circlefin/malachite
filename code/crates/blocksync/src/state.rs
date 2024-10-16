use std::collections::BTreeMap;

use derive_where::derive_where;
use libp2p::PeerId;
use malachite_common::Context;

#[derive_where(Clone, Debug, Default)]
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// Height of last decided block
    pub tip_height: Ctx::Height,

    /// Height currently syncing.
    pub sync_height: Ctx::Height,

    /// Requests for these heights have been sent out to peers.
    pub pending_requests: BTreeMap<Ctx::Height, PeerId>,

    /// The set of peers we are connected to in order to get blocks and certificates.
    pub peers: BTreeMap<PeerId, Ctx::Height>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn store_peer_height(&mut self, peer: PeerId, height: Ctx::Height) {
        self.peers.insert(peer, height);
    }

    pub fn store_pending_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_requests.insert(height, peer);
    }

    pub fn remove_pending_request(&mut self, height: Ctx::Height) {
        self.pending_requests.remove(&height);
    }
}

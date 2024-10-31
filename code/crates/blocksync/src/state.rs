use std::collections::BTreeMap;

use libp2p::PeerId;

use malachite_common::Context;
use rand::seq::IteratorRandom;

use crate::Status;

pub struct State<Ctx>
where
    Ctx: Context,
{
    rng: Box<dyn rand::RngCore + Send>,

    /// Height of last decided block
    pub tip_height: Ctx::Height,

    /// Height currently syncing.
    pub sync_height: Ctx::Height,

    /// Requests for these heights have been sent out to peers.
    pub pending_requests: BTreeMap<Ctx::Height, PeerId>,

    /// The set of peers we are connected to in order to get blocks and certificates.
    pub peers: BTreeMap<PeerId, Status<Ctx>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(rng: Box<dyn rand::RngCore + Send>, tip_height: Ctx::Height) -> Self {
        Self {
            rng,
            tip_height,
            sync_height: tip_height,
            pending_requests: BTreeMap::new(),
            peers: BTreeMap::new(),
        }
    }

    pub fn update_status(&mut self, status: Status<Ctx>) {
        self.peers.insert(status.peer_id, status);
    }

    /// Select at random a peer that that we know is at or above the given height.
    pub fn random_peer_with_block(&mut self, height: Ctx::Height) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.height >= height).then_some(peer))
            .choose_stable(&mut self.rng)
    }

    pub fn store_pending_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_requests.insert(height, peer);
    }

    pub fn remove_pending_request(&mut self, height: Ctx::Height) {
        self.pending_requests.remove(&height);
    }

    pub fn has_pending_request(&self, height: &Ctx::Height) -> bool {
        self.pending_requests.contains_key(height)
    }
}

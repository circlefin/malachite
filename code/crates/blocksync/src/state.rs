use std::collections::BTreeMap;

use libp2p::PeerId;

use malachite_common::{Context, Round};
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

    /// Decided block requests for these heights have been sent out to peers.
    pub pending_decided_block_requests: BTreeMap<Ctx::Height, PeerId>,

    /// Vote set requests for these heights and rounds have been sent out to peers.
    pub pending_vote_set_requests: BTreeMap<(Ctx::Height, Round), PeerId>,

    /// The set of peers we are connected to in order to get blocks, certificates and votes.
    /// TODO - For now block and vote sync peers are the same. Might need to revise in the future.
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
            pending_decided_block_requests: BTreeMap::new(),
            pending_vote_set_requests: BTreeMap::new(),
            peers: BTreeMap::new(),
        }
    }

    pub fn update_status(&mut self, status: Status<Ctx>) {
        self.peers.insert(status.peer_id, status);
    }

    /// Select at random a peer
    pub fn random_peer_for_votes(&mut self) -> Option<PeerId> {
        // TODO optimization - get vote sets from more than one peer
        self.peers.keys().choose_stable(&mut self.rng).cloned()
    }

    /// Select at random a peer that that we know is at or above the given height.
    pub fn random_peer_with_block(&mut self, height: Ctx::Height) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.height >= height).then_some(peer))
            .choose_stable(&mut self.rng)
    }

    /// Select at random a peer that that we know is at or above the given height,
    /// except the given one.
    pub fn random_peer_with_block_except(
        &mut self,
        height: Ctx::Height,
        except: PeerId,
    ) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.height >= height).then_some(peer))
            .filter(|&peer| peer != except)
            .choose_stable(&mut self.rng)
    }

    pub fn store_pending_decided_block_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_decided_block_requests.insert(height, peer);
    }

    pub fn remove_pending_decided_block_request(&mut self, height: Ctx::Height) {
        self.pending_decided_block_requests.remove(&height);
    }

    pub fn has_pending_decided_block_request(&self, height: &Ctx::Height) -> bool {
        self.pending_decided_block_requests.contains_key(height)
    }
    pub fn store_pending_vote_set_request(
        &mut self,
        height: Ctx::Height,
        round: Round,
        peer: PeerId,
    ) {
        self.pending_vote_set_requests.insert((height, round), peer);
    }

    pub fn remove_pending_vote_set_request(&mut self, height: Ctx::Height, round: Round) {
        self.pending_vote_set_requests.remove(&(height, round));
    }

    pub fn has_pending_vote_set_request(&self, height: Ctx::Height, round: Round) -> bool {
        self.pending_vote_set_requests
            .contains_key(&(height, round))
    }
}

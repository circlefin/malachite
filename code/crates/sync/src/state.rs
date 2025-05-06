use std::collections::BTreeMap;

use rand::seq::IteratorRandom;

use malachitebft_core_types::{Context, Height, Round};
use malachitebft_peer::PeerId;

use crate::Status;

pub struct State<Ctx>
where
    Ctx: Context,
{
    rng: Box<dyn rand::RngCore + Send>,

    /// Consensus has started
    pub started: bool,

    /// Height of last decided value
    pub tip_height: Ctx::Height,

    /// Height currently syncing.
    pub sync_height: Ctx::Height,

    /// Decided value requests for these heights have been sent out to peers.
    pub pending_decided_value_requests: BTreeMap<Ctx::Height, PeerId>,

    /// The set of peers we are connected to in order to get values, certificates and votes.
    /// TODO - For now value and vote sync peers are the same. Might need to revise in the future.
    pub peers: BTreeMap<PeerId, Status<Ctx>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(rng: Box<dyn rand::RngCore + Send>) -> Self {
        Self {
            rng,
            started: false,
            tip_height: Ctx::Height::ZERO,
            sync_height: Ctx::Height::ZERO,
            pending_decided_value_requests: BTreeMap::new(),
            peers: BTreeMap::new(),
        }
    }

    pub fn update_status(&mut self, status: Status<Ctx>) {
        self.peers.insert(status.peer_id, status);
    }

    /// Select at random a peer that is currently running consensus at `height` and round >= `round`.
    /// TODO: Potentially extend `Status` to include consensus height and round.
    pub fn random_peer_with_sync_at(
        &mut self,
        sync_height: Ctx::Height,
        _round: Round,
    ) -> Option<PeerId> {
        let tip_height = sync_height.decrement().unwrap_or(sync_height);
        self.random_peer_with_tip_at(tip_height)
    }

    /// Select at random a peer whose tip is at the given height.
    pub fn random_peer_with_tip_at(&mut self, height: Ctx::Height) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.tip_height == height).then_some(peer))
            .choose_stable(&mut self.rng)
    }

    /// Select at random a peer whose tip at or above the given height.
    pub fn random_peer_with_tip_at_or_above(&mut self, height: Ctx::Height) -> Option<PeerId>
    where
        Ctx: Context,
    {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.tip_height >= height).then_some(peer))
            .choose_stable(&mut self.rng)
    }

    /// Select at random a peer that that we know is at or above the given height, except the given one.
    pub fn random_peer_with_tip_at_or_above_except(
        &mut self,
        height: Ctx::Height,
        except: PeerId,
    ) -> Option<PeerId> {
        self.peers
            .iter()
            .filter_map(move |(&peer, status)| (status.tip_height >= height).then_some(peer))
            .filter(|&peer| peer != except)
            .choose_stable(&mut self.rng)
    }

    pub fn store_pending_decided_value_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_decided_value_requests.insert(height, peer);
    }

    pub fn remove_pending_decided_value_request(&mut self, height: Ctx::Height) {
        self.pending_decided_value_requests.remove(&height);
    }

    pub fn has_pending_decided_value_request(&self, height: &Ctx::Height) -> bool {
        self.pending_decided_value_requests.contains_key(height)
    }
}

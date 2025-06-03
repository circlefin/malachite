use std::collections::BTreeMap;
use std::time::Duration;

use malachitebft_core_types::{Context, Height};
use malachitebft_peer::PeerId;

use crate::scoring::{PeerScorer, ScoringStrategy};
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
    pub peers: BTreeMap<PeerId, Status<Ctx>>,

    /// Peer scorer for scoring peers based on their performance.
    pub peer_scorer: PeerScorer,

    /// Threshold for considering a peer inactive, and their score reset to the initial value.
    pub inactive_threshold: Option<Duration>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        // Random number generator for selecting peers
        rng: Box<dyn rand::RngCore + Send>,
        // Strategy for scoring peers based on their performance
        scoring_strategy: impl ScoringStrategy + 'static,
        // Threshold for considering a peer inactive, and their score reset to the initial value
        inactive_threshold: Option<Duration>,
    ) -> Self {
        Self {
            rng,
            started: false,
            tip_height: Ctx::Height::ZERO,
            sync_height: Ctx::Height::ZERO,
            pending_decided_value_requests: BTreeMap::new(),
            peers: BTreeMap::new(),
            peer_scorer: PeerScorer::new(scoring_strategy),
            inactive_threshold,
        }
    }

    pub fn update_status(&mut self, status: Status<Ctx>) {
        self.peers.insert(status.peer_id, status);
    }

    /// Select at random a peer whose tip is at or above the given height and with min height below the given height.
    /// In other words, `height` is in `status.history_min_height..=status.tip_height` range.
    pub fn random_peer_with_tip_at_or_above(&mut self, height: Ctx::Height) -> Option<PeerId>
    where
        Ctx: Context,
    {
        let peers = self
            .peers
            .iter()
            .filter_map(|(&peer, status)| {
                (status.history_min_height..=status.tip_height)
                    .contains(&height)
                    .then_some(peer)
            })
            .collect::<Vec<_>>();

        self.peer_scorer.select_peer(&peers, &mut self.rng)
    }

    /// Same as [`Self::random_peer_with_tip_at_or_above`], but excludes the given peer.
    pub fn random_peer_with_tip_at_or_above_except(
        &mut self,
        height: Ctx::Height,
        except: PeerId,
    ) -> Option<PeerId> {
        let peers = self
            .peers
            .iter()
            .filter_map(|(&peer, status)| {
                (status.history_min_height..=status.tip_height)
                    .contains(&height)
                    .then_some(peer)
            })
            .filter(|&peer| peer != except)
            .collect::<Vec<_>>();

        self.peer_scorer.select_peer(&peers, &mut self.rng)
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

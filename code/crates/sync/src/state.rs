use std::collections::BTreeMap;

use malachitebft_core_types::{Context, Height};
use malachitebft_peer::PeerId;

use crate::scoring::{ema, PeerScorer, Strategy};
use crate::{Config, OutboundRequestId, Status};

/// State of a decided value request.
///
/// State transitions:
/// WaitingResponse -> WaitingValidation -> Validated
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestState {
    /// Initial state: waiting for a response from a peer
    WaitingResponse,
    /// Response received: waiting for value validation by consensus
    WaitingValidation,
    /// Value validated: request is complete
    Validated,
}

pub struct State<Ctx>
where
    Ctx: Context,
{
    rng: Box<dyn rand::RngCore + Send>,

    /// Configuration for the sync state and behaviour.
    pub config: Config,

    /// Consensus has started
    pub started: bool,

    /// Height of last decided value
    pub tip_height: Ctx::Height,

    /// Height currently syncing.
    pub sync_height: Ctx::Height,

    /// Decided value requests for these heights have been sent out to peers.
    pub pending_value_requests: BTreeMap<Ctx::Height, (OutboundRequestId, PeerId, RequestState)>,

    /// Maps request ID to height for pending decided value requests.
    pub height_per_request_id: BTreeMap<OutboundRequestId, (Ctx::Height, PeerId)>,

    /// The set of peers we are connected to in order to get values, certificates and votes.
    pub peers: BTreeMap<PeerId, Status<Ctx>>,

    /// Peer scorer for scoring peers based on their performance.
    pub peer_scorer: PeerScorer,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        // Random number generator for selecting peers
        rng: Box<dyn rand::RngCore + Send>,
        // Sync configuration
        config: Config,
    ) -> Self {
        let peer_scorer = match config.scoring_strategy {
            Strategy::Ema => PeerScorer::new(ema::ExponentialMovingAverage::default()),
        };

        Self {
            rng,
            config,
            started: false,
            tip_height: Ctx::Height::ZERO,
            sync_height: Ctx::Height::ZERO,
            pending_value_requests: BTreeMap::new(),
            height_per_request_id: BTreeMap::new(),
            peers: BTreeMap::new(),
            peer_scorer,
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

    /// Store a pending decided value request for a given height and request ID.
    ///
    /// State transition: None -> WaitingResponse
    pub fn store_pending_value_request(
        &mut self,
        height: Ctx::Height,
        request_id: OutboundRequestId,
        peer_id: PeerId,
    ) {
        self.height_per_request_id
            .insert(request_id.clone(), (height, peer_id));

        self.pending_value_requests
            .insert(height, (request_id, peer_id, RequestState::WaitingResponse));
    }

    /// Mark that a response has been received for a height.
    ///
    /// State transition: WaitingResponse -> WaitingValidation
    pub fn response_received(
        &mut self,
        request_id: OutboundRequestId,
        height: Ctx::Height,
        peer_id: PeerId,
    ) {
        if let Some((req_id, stored_peer_id, state)) = self.pending_value_requests.get_mut(&height)
        {
            if req_id != &request_id || stored_peer_id != &peer_id {
                return; // A new request has been made in the meantime, ignore this response.
            }
            if *state == RequestState::WaitingResponse {
                *state = RequestState::WaitingValidation;
            }
        }
    }

    /// Mark that a decided value has been validated for a height.
    ///
    /// State transition: WaitingValidation -> Validated
    /// It is also possible to have the following transition: WaitingResponse -> Validated.
    pub fn validate_response(&mut self, height: Ctx::Height) {
        if let Some((_, _, state)) = self.pending_value_requests.get_mut(&height) {
            *state = RequestState::Validated;
        }
    }

    /// Get the height for a given request ID.
    pub fn get_height_for_request_id(
        &self,
        request_id: &OutboundRequestId,
    ) -> Option<(Ctx::Height, PeerId)> {
        self.height_per_request_id.get(request_id).cloned()
    }

    /// Remove the pending decided value request for a given height.
    pub fn remove_pending_request_by_height(&mut self, height: &Ctx::Height) {
        if let Some((request_id, _, _)) = self.pending_value_requests.remove(height) {
            self.height_per_request_id.remove(&request_id);
        }
    }

    /// Remove a pending decided value request by its ID and return the height and peer it was associated with.
    pub fn remove_pending_value_request_by_id(
        &mut self,
        request_id: &OutboundRequestId,
    ) -> Option<(Ctx::Height, PeerId)> {
        let (height, peer_id) = self.height_per_request_id.remove(request_id)?;

        self.pending_value_requests.remove(&height);

        Some((height, peer_id))
    }

    /// Check if there are any pending decided value requests for a given height.
    pub fn has_pending_value_request(&self, height: &Ctx::Height) -> bool {
        self.pending_value_requests.contains_key(height)
    }

    /// Check if a pending decided value request for a given height is in the `Validated` state.
    pub fn is_pending_value_request_validated_by_height(&self, height: &Ctx::Height) -> bool {
        if let Some((_, _, state)) = self.pending_value_requests.get(height) {
            *state == RequestState::Validated
        } else {
            false
        }
    }

    /// Check if a pending decided value request for a given request ID is in the `Validated` state.
    pub fn is_pending_value_request_validated_by_id(&self, request_id: &OutboundRequestId) -> bool {
        if let Some((height, _)) = self.height_per_request_id.get(request_id) {
            self.is_pending_value_request_validated_by_height(height)
        } else {
            false
        }
    }
}

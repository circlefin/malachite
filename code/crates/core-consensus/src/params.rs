use derive_where::derive_where;

use malachitebft_core_types::{Context, ValuePayload};

pub use malachitebft_core_driver::ThresholdParams;

/// Consensus parameters.
#[derive_where(Clone, Debug)]
pub struct Params<Ctx: Context> {
    /// The initial height
    pub initial_height: Ctx::Height,

    /// The initial validator set
    pub initial_validator_set: Ctx::ValidatorSet,

    /// The address of this validator
    pub address: Ctx::Address,

    /// The quorum and honest thresholds
    pub threshold_params: ThresholdParams,

    /// The messages required to deliver proposals
    pub value_payload: ValuePayload,

    /// The VoteSync mode
    pub vote_sync_mode: VoteSyncMode,
}

/// The mode of vote synchronization
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoteSyncMode {
    /// The lagging node sends a request to a peer for the missing votes
    #[default]
    RequestResponse,
    /// Nodes rebroadcast their last vote to all peers
    Rebroadcast,
}

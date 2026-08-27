use derive_where::derive_where;

use malachitebft_core_types::{Context, Round, ValuePayload};

/// The round from which we enable the hidden lock mitigation mechanism
pub const HIDDEN_LOCK_ROUND: Round = Round::new(10);

/// Maximum number of rounds ahead of the current consensus round from which
/// votes are accepted at the current height.
///
/// Votes whose round exceeds `current_round + MAX_FUTURE_ROUND_LOOKAHEAD` are
/// dropped before signature verification and WAL append, bounding per-height
/// vote-keeper state, signature verification work, and WAL I/O regardless of
/// the round numbers carried by incoming votes.
///
/// The bound still allows the `SkipRound` mechanism (`f+1` honest votes at a
/// higher round trigger a round skip) to catch up progressively: as the
/// current round advances, the ceiling slides with it, so votes that were
/// previously out of range become acceptable.
pub const MAX_FUTURE_ROUND_LOOKAHEAD: u32 = 10;

#[doc(inline)]
pub use malachitebft_core_driver::ThresholdParams;

/// Consensus parameters.
#[derive_where(Clone, Debug)]
pub struct Params<Ctx: Context> {
    /// The address of this validator
    pub address: Ctx::Address,

    /// The quorum and honest thresholds
    pub threshold_params: ThresholdParams,

    /// The messages required to deliver proposals
    pub value_payload: ValuePayload,

    /// Whether consensus is enabled for this node
    pub enabled: bool,
}

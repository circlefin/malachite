use derive_where::derive_where;

use malachitebft_core_types::{Context, Round, ValuePayload};

/// The round from which we enable the hidden lock mitigation mechanism
pub const HIDDEN_LOCK_ROUND: Round = Round::new(10);

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
}

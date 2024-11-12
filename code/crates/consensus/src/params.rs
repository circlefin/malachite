use derive_where::derive_where;
use malachite_common::Context;

pub use malachite_driver::ThresholdParams;

use crate::ValuePayload;

/// Consensus parameters.
#[derive_where(Clone, Debug)]
pub struct Params<Ctx: Context> {
    /// The starting height
    pub start_height: Ctx::Height,

    /// The initial validator set
    pub initial_validator_set: Ctx::ValidatorSet,

    /// The address of this validator
    pub address: Ctx::Address,

    /// The quorum and honest thresholds
    pub threshold_params: ThresholdParams,

    /// The messages required to deliver proposals
    pub value_payload: ValuePayload,
}

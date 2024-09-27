use derive_where::derive_where;
use malachite_common::Context;

pub use malachite_driver::ThresholdParams;

#[derive_where(Clone, Debug)]
pub struct Params<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub initial_validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
}

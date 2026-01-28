use derive_where::derive_where;

use crate::Context;

/// Consensus parameters to use when starting or restarting a height.
#[derive_where(Debug, Clone, PartialEq, Eq)]
pub struct HeightParams<Ctx: Context> {
    /// Validator set for the height
    pub validator_set: Ctx::ValidatorSet,

    /// Timeouts for the height
    pub timeouts: Ctx::Timeouts,
}

impl<Ctx: Context> HeightParams<Ctx> {
    /// Create new height parameters.
    pub fn new(validator_set: Ctx::ValidatorSet, timeouts: Ctx::Timeouts) -> Self {
        Self {
            validator_set,
            timeouts,
        }
    }
}

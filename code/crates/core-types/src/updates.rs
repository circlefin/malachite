use derive_where::derive_where;

use crate::Context;

/// Updates to apply when starting or restarting a height.
#[derive_where(Debug, Clone, PartialEq, Eq, Default)]
pub struct Updates<Ctx: Context> {
    /// Validator set for the height
    ///
    /// If `None`, the validator set will be the same as the current validator set.
    pub validator_set: Option<Ctx::ValidatorSet>,
    /// Optional timeouts override for the height
    ///
    /// If `None`, the timeouts will be the same as the current timeouts.
    pub timeouts: Option<Ctx::Timeouts>,
}

impl<Ctx: Context> Updates<Ctx> {
    /// Apply a validator set update to the height updates.
    pub fn with_validator_set(mut self, validator_set: Ctx::ValidatorSet) -> Self {
        self.validator_set = Some(validator_set);
        self
    }

    /// Apply a timeouts update to the height updates.
    pub fn with_timeouts(mut self, timeouts: Ctx::Timeouts) -> Self {
        self.timeouts = Some(timeouts);
        self
    }
}

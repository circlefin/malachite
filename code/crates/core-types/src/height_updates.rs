//! Height updates structure for starting or restarting a height.

use derive_where::derive_where;

use crate::Context;

/// Updates to apply when starting or restarting a height.
#[derive_where(Debug, Clone, PartialEq, Eq)]
pub struct HeightUpdates<Ctx: Context> {
    /// Validator set for the height
    ///
    /// If `None`, the validator set will be the same as the current validator set.
    pub validator_set: Option<Ctx::ValidatorSet>,
    /// Optional timeouts override for the height
    ///
    /// If `None`, the timeouts will be the same as the current timeouts.
    pub timeouts: Option<Ctx::Timeouts>,
}

impl<Ctx: Context> HeightUpdates<Ctx> {
    /// Create a new `HeightUpdates` struct with no updates.
    pub fn none() -> Self {
        Self {
            validator_set: None,
            timeouts: None,
        }
    }
}

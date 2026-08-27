use core::time::Duration;
use derive_where::derive_where;

use crate::{Context, VoteExtensionPolicy};

/// Consensus parameters to use when starting or restarting a height.
#[derive_where(Debug, Clone, PartialEq, Eq)]
pub struct HeightParams<Ctx: Context> {
    /// Validator set for the height
    pub validator_set: Ctx::ValidatorSet,

    /// Timeouts for the height
    pub timeouts: Ctx::Timeouts,

    /// Target time for this height
    pub target_time: Option<Duration>,

    /// Vote-extension verification policy for this height.
    pub vote_extension_policy: VoteExtensionPolicy,
}

impl<Ctx: Context> HeightParams<Ctx> {
    /// Create new height parameters.
    pub fn new(
        validator_set: Ctx::ValidatorSet,
        timeouts: Ctx::Timeouts,
        target_time: Option<Duration>,
    ) -> Self {
        Self {
            validator_set,
            timeouts,
            target_time,
            vote_extension_policy: VoteExtensionPolicy::default(),
        }
    }

    /// Create new height parameters with an explicit vote-extension policy.
    pub fn with_vote_extension_policy(
        mut self,
        vote_extension_policy: VoteExtensionPolicy,
    ) -> Self {
        self.vote_extension_policy = vote_extension_policy;
        self
    }
}

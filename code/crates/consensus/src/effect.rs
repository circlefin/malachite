use derive_where::derive_where;

use malachite_common::*;

use crate::types::GossipMsg;

pub type Yielder<Ctx> = corosensei::Yielder<Resume<Ctx>, Effect<Ctx>>;

#[must_use]
#[derive_where(Debug)]
pub enum Effect<Ctx>
where
    Ctx: Context,
{
    /// Reset all timeouts
    /// Resume with: Resume::Continue
    ResetTimeouts,

    /// Cancel all timeouts
    /// Resume with: Resume::Continue
    CancelAllTimeouts,

    /// Cancel a given timeout
    /// Resume with: Resume::Continue
    CancelTimeout(Timeout),

    /// Schedule a timeout
    /// Resume with: Resume::Continue
    ScheduleTimeout(Timeout),

    /// Broadcast a message
    /// Resume with: Resume::Continue
    Broadcast(GossipMsg<Ctx>),

    /// Get a value to propose at the given height and round, within the given timeout
    /// Resume with: Resume::ProposeValue(height, round, value)
    GetValue(Ctx::Height, Round, Timeout),

    /// Get the validator set at the given height
    /// Resume with: Resume::ValidatorSet(height, validator_set)
    GetValidatorSet(Ctx::Height),

    /// Consensus has decided on a value
    /// Resume with: Resume::Continue
    DecidedOnValue {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedVote<Ctx>>,
    },

    /// A BlockPart was received via the gossip layer
    /// Resume with: Resume::Continue
    ReceivedBlockPart(Ctx::BlockPart),
}

#[must_use]
#[derive_where(Debug)]
pub enum Resume<Ctx>
where
    Ctx: Context,
{
    Start,
    Continue,
    ProposeValue(Ctx::Height, Round, Ctx::Value),
    ValidatorSet(Ctx::Height, Ctx::ValidatorSet),
}

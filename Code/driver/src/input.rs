use malachite_common::{Context, Round, SignedVote, Timeout};

use crate::Validity;

/// Events that can be received by the [`Driver`](crate::Driver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Propose a value for the given round
    ProposeValue(Round, Ctx::Value),

    /// Receive a proposal, of the given validity
    Proposal(Ctx::Proposal, Validity),

    /// Receive a signed vote
    Vote(SignedVote<Ctx>),

    /// Receive a timeout
    TimeoutElapsed(Timeout),
}

use malachite_common::{Context, Round, SignedVote, Timeout};

use crate::Validity;

/// Events that can be received by the [`Driver`](crate::Driver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<Ctx>
where
    Ctx: Context,
{
    /// A new round has started.
    /// The boolean indicates whether we are the proposer or not.
    NewRound(Ctx::Height, Round),

    /// A new proposal has been received.
    Proposal(Ctx::Proposal, Validity),

    /// A new vote has been received.
    Vote(SignedVote<Ctx>),

    /// A timeout has elapsed.
    TimeoutElapsed(Timeout),
}

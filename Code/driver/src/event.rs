use malachite_common::{Context, Round, SignedVote, Timeout};

use crate::Validity;

/// Events that can be received by the [`Driver`](crate::Driver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<Ctx>
where
    Ctx: Context,
{
    NewRound(Round),
    Proposal(Ctx::Proposal, Validity),
    Vote(SignedVote<Ctx>),
    TimeoutElapsed(Timeout),
}

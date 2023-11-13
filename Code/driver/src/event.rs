use malachite_common::{Context, Round, SignedVote, Timeout};

/// Events that can be received by the [`Driver`](crate::Driver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<Ctx>
where
    Ctx: Context,
{
    NewRound(Round),
    Proposal(Ctx::Proposal),
    Vote(SignedVote<Ctx>),
    TimeoutElapsed(Timeout),
}

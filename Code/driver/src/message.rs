use malachite_common::{Context, Round, SignedVote, Timeout};

/// Messages emitted by the [`Driver`](crate::Driver)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<Ctx>
where
    Ctx: Context,
{
    Propose(Ctx::Proposal),
    Vote(SignedVote<Ctx>),
    Decide(Round, Ctx::Value),
    ScheduleTimeout(Timeout),
    NewRound(Round),
}

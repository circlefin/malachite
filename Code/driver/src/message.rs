use core::fmt;

use malachite_common::{Context, Round, SignedVote, Timeout};

/// Messages emitted by the [`Driver`](crate::Driver)
pub enum Message<Ctx>
where
    Ctx: Context,
{
    Propose(Ctx::Proposal),
    Vote(SignedVote<Ctx>),
    Decide(Round, Ctx::Value),
    ScheduleTimeout(Timeout),
    NewRound(Ctx::Height, Round),
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Message::Propose(proposal) => Message::Propose(proposal.clone()),
            Message::Vote(signed_vote) => Message::Vote(signed_vote.clone()),
            Message::Decide(round, value) => Message::Decide(*round, value.clone()),
            Message::ScheduleTimeout(timeout) => Message::ScheduleTimeout(*timeout),
            Message::NewRound(height, round) => Message::NewRound(height.clone(), *round),
        }
    }
}

impl<Ctx: Context> fmt::Debug for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Propose(proposal) => write!(f, "Propose({:?})", proposal),
            Message::Vote(signed_vote) => write!(f, "Vote({:?})", signed_vote),
            Message::Decide(round, value) => write!(f, "Decide({:?}, {:?})", round, value),
            Message::ScheduleTimeout(timeout) => write!(f, "ScheduleTimeout({:?})", timeout),
            Message::NewRound(height, round) => write!(f, "NewRound({:?}, {:?})", height, round),
        }
    }
}

impl<Ctx: Context> PartialEq for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Message::Propose(proposal), Message::Propose(other_proposal)) => {
                proposal == other_proposal
            }
            (Message::Vote(signed_vote), Message::Vote(other_signed_vote)) => {
                signed_vote == other_signed_vote
            }
            (Message::Decide(round, value), Message::Decide(other_round, other_value)) => {
                round == other_round && value == other_value
            }
            (Message::ScheduleTimeout(timeout), Message::ScheduleTimeout(other_timeout)) => {
                timeout == other_timeout
            }
            (Message::NewRound(height, round), Message::NewRound(other_height, other_round)) => {
                height == other_height && round == other_round
            }
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Message<Ctx> {}

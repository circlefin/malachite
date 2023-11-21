use core::fmt;

use malachite_common::{Context, Round, SignedVote, Timeout};

/// Messages emitted by the [`Driver`](crate::Driver)
pub enum Message<Ctx>
where
    Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Broadcast a proposal
    Propose(Ctx::Proposal),

    /// Broadcast a vote for a value
    Vote(SignedVote<Ctx>),

    /// Decide on a value
    Decide(Round, Ctx::Value),

    /// Schedule a timeout
    ScheduleTimeout(Timeout),

    /// Ask for a value to propose and schedule a timeout
    GetValueAndScheduleTimeout(Round, Timeout),
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Message::NewRound(height, round) => Message::NewRound(height.clone(), *round),
            Message::Propose(proposal) => Message::Propose(proposal.clone()),
            Message::Vote(signed_vote) => Message::Vote(signed_vote.clone()),
            Message::Decide(round, value) => Message::Decide(*round, value.clone()),
            Message::ScheduleTimeout(timeout) => Message::ScheduleTimeout(*timeout),
            Message::GetValueAndScheduleTimeout(round, timeout) => {
                Message::GetValueAndScheduleTimeout(*round, *timeout)
            }
        }
    }
}

impl<Ctx: Context> fmt::Debug for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::NewRound(height, round) => write!(f, "NewRound({:?}, {:?})", height, round),
            Message::Propose(proposal) => write!(f, "Propose({:?})", proposal),
            Message::Vote(signed_vote) => write!(f, "Vote({:?})", signed_vote),
            Message::Decide(round, value) => write!(f, "Decide({:?}, {:?})", round, value),
            Message::ScheduleTimeout(timeout) => write!(f, "ScheduleTimeout({:?})", timeout),
            Message::GetValueAndScheduleTimeout(round, timeout) => {
                write!(f, "GetValueAndScheduleTimeout({:?}, {:?})", round, timeout)
            }
        }
    }
}

impl<Ctx: Context> PartialEq for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Message::NewRound(height, round), Message::NewRound(other_height, other_round)) => {
                height == other_height && round == other_round
            }
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
            (
                Message::GetValueAndScheduleTimeout(round, timeout),
                Message::GetValueAndScheduleTimeout(other_round, other_timeout),
            ) => round == other_round && timeout == other_timeout,
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Message<Ctx> {}

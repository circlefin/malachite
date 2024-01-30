use core::fmt;

use malachite_common::{Context, Round, Timeout};

/// Messages emitted by the [`Driver`](crate::Driver)
pub enum Output<Ctx>
where
    Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Broadcast a proposal
    Propose(Ctx::Proposal),

    /// Broadcast a vote for a value
    Vote(Ctx::Vote),

    /// Decide on a value
    Decide(Round, Ctx::Value),

    /// Schedule a timeout
    ScheduleTimeout(Timeout),

    /// Ask for a value at the given height, round.
    /// The timeout tells the proposal builder how long it has to build a value.
    GetValue(Ctx::Height, Round, Timeout),
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Output::NewRound(height, round) => Output::NewRound(*height, *round),
            Output::Propose(proposal) => Output::Propose(proposal.clone()),
            Output::Vote(signed_vote) => Output::Vote(signed_vote.clone()),
            Output::Decide(round, value) => Output::Decide(*round, value.clone()),
            Output::ScheduleTimeout(timeout) => Output::ScheduleTimeout(*timeout),
            Output::GetValue(height, round, timeout) => Output::GetValue(*height, *round, *timeout),
        }
    }
}

impl<Ctx: Context> fmt::Debug for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Output::NewRound(height, round) => write!(f, "NewRound({:?}, {:?})", height, round),
            Output::Propose(proposal) => write!(f, "Propose({:?})", proposal),
            Output::Vote(signed_vote) => write!(f, "Vote({:?})", signed_vote),
            Output::Decide(round, value) => write!(f, "Decide({:?}, {:?})", round, value),
            Output::ScheduleTimeout(timeout) => write!(f, "ScheduleTimeout({:?})", timeout),
            Output::GetValue(height, round, timeout) => {
                write!(f, "GetValue({:?}, {:?}, {:?})", height, round, timeout)
            }
        }
    }
}

impl<Ctx: Context> PartialEq for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Output::NewRound(height, round), Output::NewRound(other_height, other_round)) => {
                height == other_height && round == other_round
            }
            (Output::Propose(proposal), Output::Propose(other_proposal)) => {
                proposal == other_proposal
            }
            (Output::Vote(signed_vote), Output::Vote(other_signed_vote)) => {
                signed_vote == other_signed_vote
            }
            (Output::Decide(round, value), Output::Decide(other_round, other_value)) => {
                round == other_round && value == other_value
            }
            (Output::ScheduleTimeout(timeout), Output::ScheduleTimeout(other_timeout)) => {
                timeout == other_timeout
            }
            (
                Output::GetValue(height, round, timeout),
                Output::GetValue(other_height, other_round, other_timeout),
            ) => height == other_height && round == other_round && timeout == other_timeout,
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Output<Ctx> {}

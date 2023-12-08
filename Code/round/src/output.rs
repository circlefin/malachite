use core::fmt;

use malachite_common::{Context, NilOrVal, Round, Timeout, TimeoutStep, ValueId};

use crate::state::RoundValue;

pub enum Output<Ctx>
where
    Ctx: Context,
{
    NewRound(Round),                            // Move to the new round.
    Proposal(Ctx::Proposal),                    // Broadcast the proposal.
    Vote(Ctx::Vote),                            // Broadcast the vote.
    ScheduleTimeout(Timeout),                   // Schedule the timeout.
    GetValueAndScheduleTimeout(Round, Timeout), // Ask for a value and schedule a timeout.
    Decision(RoundValue<Ctx::Value>),           // Decide the value.
}

impl<Ctx: Context> Output<Ctx> {
    pub fn proposal(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        pol_round: Round,
    ) -> Self {
        Output::Proposal(Ctx::new_proposal(height, round, value, pol_round))
    }

    pub fn prevote(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_prevote(height, round, value_id, address))
    }

    pub fn precommit(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_precommit(height, round, value_id, address))
    }

    pub fn schedule_timeout(round: Round, step: TimeoutStep) -> Self {
        Output::ScheduleTimeout(Timeout { round, step })
    }

    pub fn get_value_and_schedule_timeout(round: Round, step: TimeoutStep) -> Self {
        Output::GetValueAndScheduleTimeout(round, Timeout { round, step })
    }

    pub fn decision(round: Round, value: Ctx::Value) -> Self {
        Output::Decision(RoundValue { round, value })
    }
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Output::NewRound(round) => Output::NewRound(*round),
            Output::Proposal(proposal) => Output::Proposal(proposal.clone()),
            Output::Vote(vote) => Output::Vote(vote.clone()),
            Output::ScheduleTimeout(timeout) => Output::ScheduleTimeout(*timeout),
            Output::GetValueAndScheduleTimeout(round, timeout) => {
                Output::GetValueAndScheduleTimeout(*round, *timeout)
            }
            Output::Decision(round_value) => Output::Decision(round_value.clone()),
        }
    }
}

impl<Ctx: Context> fmt::Debug for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Output::NewRound(round) => write!(f, "NewRound({:?})", round),
            Output::Proposal(proposal) => write!(f, "Proposal({:?})", proposal),
            Output::Vote(vote) => write!(f, "Vote({:?})", vote),
            Output::ScheduleTimeout(timeout) => write!(f, "ScheduleTimeout({:?})", timeout),
            Output::GetValueAndScheduleTimeout(round, timeout) => {
                write!(f, "GetValueAndScheduleTimeout({:?}, {:?})", round, timeout)
            }
            Output::Decision(round_value) => write!(f, "Decision({:?})", round_value),
        }
    }
}

impl<Ctx: Context> PartialEq for Output<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Output::NewRound(round), Output::NewRound(other_round)) => round == other_round,
            (Output::Proposal(proposal), Output::Proposal(other_proposal)) => {
                proposal == other_proposal
            }
            (Output::Vote(vote), Output::Vote(other_vote)) => vote == other_vote,
            (Output::ScheduleTimeout(timeout), Output::ScheduleTimeout(other_timeout)) => {
                timeout == other_timeout
            }
            (
                Output::GetValueAndScheduleTimeout(round, timeout),
                Output::GetValueAndScheduleTimeout(other_round, other_timeout),
            ) => round == other_round && timeout == other_timeout,
            (Output::Decision(round_value), Output::Decision(other_round_value)) => {
                round_value == other_round_value
            }
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Output<Ctx> {}

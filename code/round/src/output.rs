//! Outputs of the round state machine.

use core::fmt;

use malachite_common::{Context, NilOrVal, Round, Timeout, TimeoutStep, ValueId};

use crate::state::RoundValue;

/// Output of the round state machine.
pub enum Output<Ctx>
where
    Ctx: Context,
{
    /// Move to the new round.
    NewRound(Round),

    /// Broadcast the proposal.
    Proposal(Ctx::Proposal),

    /// Broadcast the vote.
    Vote(Ctx::Vote),

    /// Schedule the timeout.
    ScheduleTimeout(Timeout),

    /// Ask for a value at the given height, round and to schedule a timeout.
    /// The timeout tells the proposal builder how long it has to build a value.
    GetValueAndScheduleTimeout(Ctx::Height, Round, Timeout),

    /// Decide the value.
    Decision(RoundValue<Ctx::Value>),
}

impl<Ctx: Context> Output<Ctx> {
    /// Build a `Proposal` output.
    pub fn proposal(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        pol_round: Round,
    ) -> Self {
        Output::Proposal(Ctx::new_proposal(height, round, value, pol_round))
    }

    /// Build a `Vote` output for a prevote.
    pub fn prevote(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_prevote(height, round, value_id, address))
    }

    /// Build a `Vote` output for a precommit.
    pub fn precommit(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_precommit(height, round, value_id, address))
    }

    /// Build a `ScheduleTimeout` output.
    pub fn schedule_timeout(round: Round, step: TimeoutStep) -> Self {
        Output::ScheduleTimeout(Timeout { round, step })
    }

    /// Build a `GetValue` output.
    pub fn get_value_and_schedule_timeout(
        height: Ctx::Height,
        round: Round,
        step: TimeoutStep,
    ) -> Self {
        Output::GetValueAndScheduleTimeout(height, round, Timeout { round, step })
    }

    /// Build a `Decision` output.
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
            Output::GetValueAndScheduleTimeout(height, round, timeout) => {
                Output::GetValueAndScheduleTimeout(*height, *round, *timeout)
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
            Output::GetValueAndScheduleTimeout(height, round, timeout) => {
                write!(
                    f,
                    "GetValueAndScheduleTimeout({:?}, {:?}, {:?})",
                    height, round, timeout
                )
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
                Output::GetValueAndScheduleTimeout(height, round, timeout),
                Output::GetValueAndScheduleTimeout(other_height, other_round, other_timeout),
            ) => height == other_height && round == other_round && timeout == other_timeout,
            (Output::Decision(round_value), Output::Decision(other_round_value)) => {
                round_value == other_round_value
            }
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Output<Ctx> {}

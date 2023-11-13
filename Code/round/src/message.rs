use core::fmt;

use malachite_common::{Context, Round, Timeout, TimeoutStep, ValueId};

use crate::state::RoundValue;

pub enum Message<Ctx>
where
    Ctx: Context,
{
    NewRound(Round),                  // Move to the new round.
    Proposal(Ctx::Proposal),          // Broadcast the proposal.
    Vote(Ctx::Vote),                  // Broadcast the vote.
    ScheduleTimeout(Timeout),         // Schedule the timeout.
    Decision(RoundValue<Ctx::Value>), // Decide the value.
}

impl<Ctx: Context> Message<Ctx> {
    pub fn proposal(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        pol_round: Round,
    ) -> Self {
        Message::Proposal(Ctx::new_proposal(height, round, value, pol_round))
    }

    pub fn prevote(round: Round, value_id: Option<ValueId<Ctx>>, address: Ctx::Address) -> Self {
        Message::Vote(Ctx::new_prevote(round, value_id, address))
    }

    pub fn precommit(round: Round, value_id: Option<ValueId<Ctx>>, address: Ctx::Address) -> Self {
        Message::Vote(Ctx::new_precommit(round, value_id, address))
    }

    pub fn schedule_timeout(round: Round, step: TimeoutStep) -> Self {
        Message::ScheduleTimeout(Timeout { round, step })
    }

    pub fn decision(round: Round, value: Ctx::Value) -> Self {
        Message::Decision(RoundValue { round, value })
    }
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Message::NewRound(round) => Message::NewRound(*round),
            Message::Proposal(proposal) => Message::Proposal(proposal.clone()),
            Message::Vote(vote) => Message::Vote(vote.clone()),
            Message::ScheduleTimeout(timeout) => Message::ScheduleTimeout(*timeout),
            Message::Decision(round_value) => Message::Decision(round_value.clone()),
        }
    }
}

impl<Ctx: Context> fmt::Debug for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::NewRound(round) => write!(f, "NewRound({:?})", round),
            Message::Proposal(proposal) => write!(f, "Proposal({:?})", proposal),
            Message::Vote(vote) => write!(f, "Vote({:?})", vote),
            Message::ScheduleTimeout(timeout) => write!(f, "ScheduleTimeout({:?})", timeout),
            Message::Decision(round_value) => write!(f, "Decision({:?})", round_value),
        }
    }
}

impl<Ctx: Context> PartialEq for Message<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Message::NewRound(round), Message::NewRound(other_round)) => round == other_round,
            (Message::Proposal(proposal), Message::Proposal(other_proposal)) => {
                proposal == other_proposal
            }
            (Message::Vote(vote), Message::Vote(other_vote)) => vote == other_vote,
            (Message::ScheduleTimeout(timeout), Message::ScheduleTimeout(other_timeout)) => {
                timeout == other_timeout
            }
            (Message::Decision(round_value), Message::Decision(other_round_value)) => {
                round_value == other_round_value
            }
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Message<Ctx> {}

use malachite_common::{Context, Round, Timeout, TimeoutStep, ValueId};

use crate::state::RoundValue;

#[derive(Debug, PartialEq, Eq)]
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

impl<Ctx> Clone for Message<Ctx>
where
    Ctx: Context,
{
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

impl<Ctx> Message<Ctx>
where
    Ctx: Context,
{
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

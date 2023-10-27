use malachite_common::{Consensus, Round, Timeout, TimeoutStep, ValueId};

use crate::state::RoundValue;

#[derive(Debug, PartialEq, Eq)]
pub enum Message<C>
where
    C: Consensus,
{
    NewRound(Round),                // Move to the new round.
    Proposal(C::Proposal),          // Broadcast the proposal.
    Vote(C::Vote),                  // Broadcast the vote.
    Timeout(Timeout),               // Schedule the timeout.
    Decision(RoundValue<C::Value>), // Decide the value.
}

impl<C> Clone for Message<C>
where
    C: Consensus,
{
    fn clone(&self) -> Self {
        match self {
            Message::NewRound(round) => Message::NewRound(*round),
            Message::Proposal(proposal) => Message::Proposal(proposal.clone()),
            Message::Vote(vote) => Message::Vote(vote.clone()),
            Message::Timeout(timeout) => Message::Timeout(*timeout),
            Message::Decision(round_value) => Message::Decision(round_value.clone()),
        }
    }
}

impl<C> Message<C>
where
    C: Consensus,
{
    pub fn proposal(height: C::Height, round: Round, value: C::Value, pol_round: Round) -> Self {
        Message::Proposal(C::new_proposal(height, round, value, pol_round))
    }

    pub fn prevote(round: Round, value_id: Option<ValueId<C>>) -> Self {
        Message::Vote(C::new_prevote(round, value_id))
    }

    pub fn precommit(round: Round, value_id: Option<ValueId<C>>) -> Self {
        Message::Vote(C::new_precommit(round, value_id))
    }

    pub fn timeout(round: Round, step: TimeoutStep) -> Self {
        Message::Timeout(Timeout { round, step })
    }

    pub fn decision(round: Round, value: C::Value) -> Self {
        Message::Decision(RoundValue { round, value })
    }
}

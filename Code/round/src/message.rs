use crate::{state::RoundValue, Proposal, Round, Timeout, TimeoutStep, Value, Vote};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    NewRound(Round),      // Move to the new round.
    Proposal(Proposal),   // Broadcast the proposal.
    Vote(Vote),           // Broadcast the vote.
    Timeout(Timeout),     // Schedule the timeout.
    Decision(RoundValue), // Decide the value.
}

impl Message {
    pub fn proposal(round: Round, value: Value, pol_round: Round) -> Message {
        Message::Proposal(Proposal {
            round,
            value,
            pol_round,
        })
    }

    pub fn prevote(round: Round, value: Option<Value>) -> Message {
        Message::Vote(Vote::new_prevote(round, value))
    }

    pub fn precommit(round: Round, value: Option<Value>) -> Message {
        Message::Vote(Vote::new_precommit(round, value))
    }

    pub fn timeout(round: Round, step: TimeoutStep) -> Message {
        Message::Timeout(Timeout { round, step })
    }

    pub fn decision(round: Round, value: Value) -> Message {
        Message::Decision(RoundValue { round, value })
    }
}

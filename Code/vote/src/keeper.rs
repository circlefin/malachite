use alloc::collections::BTreeMap;

use malachite_common::{Height, Round, Vote, VoteType};
use malachite_round::events::Event;

use crate::{
    count::{Threshold, Weight},
    RoundVotes,
};

/// Keeps track of votes and emits events when thresholds are reached.
#[derive(Clone, Debug)]
pub struct VoteKeeper {
    height: Height,
    total_weight: Weight,
    rounds: BTreeMap<Round, RoundVotes>,
}

impl VoteKeeper {
    pub fn new(height: Height, total_weight: Weight) -> Self {
        let mut rounds = BTreeMap::new();

        rounds.insert(
            Round::INITIAL,
            RoundVotes::new(height, Round::INITIAL, total_weight),
        );

        VoteKeeper {
            height,
            total_weight,
            rounds,
        }
    }

    /// Apply a vote. If it triggers an event, apply the event to the state machine,
    /// returning the new state and any resulting message.
    pub fn apply(&mut self, vote: Vote, weight: Weight) -> Option<Event> {
        let round = self
            .rounds
            .entry(vote.round)
            .or_insert_with(|| RoundVotes::new(self.height, vote.round, self.total_weight));

        let vote_type = vote.typ;
        let threshold = round.add_vote(vote, weight);

        Self::to_event(vote_type, threshold)
    }

    /// Map a vote type and a threshold to a state machine event.
    fn to_event(typ: VoteType, threshold: Threshold) -> Option<Event> {
        match (typ, threshold) {
            (_, Threshold::Init) => None,

            (VoteType::Prevote, Threshold::Any) => Some(Event::PolkaAny),
            (VoteType::Prevote, Threshold::Nil) => Some(Event::PolkaNil),
            (VoteType::Prevote, Threshold::Value(v)) => Some(Event::PolkaValue(v.as_ref().clone())),

            (VoteType::Precommit, Threshold::Any) => Some(Event::PrecommitAny),
            (VoteType::Precommit, Threshold::Nil) => None,
            (VoteType::Precommit, Threshold::Value(v)) => {
                Some(Event::PrecommitValue(v.as_ref().clone()))
            }
        }
    }
}

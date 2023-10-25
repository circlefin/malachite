use alloc::collections::BTreeMap;

use malachite_common::{Consensus, Round, ValueId, Vote, VoteType};
use malachite_round::events::Event;

use crate::{
    count::{Threshold, Weight},
    RoundVotes,
};

/// Keeps track of votes and emits events when thresholds are reached.
#[derive(Clone, Debug)]
pub struct VoteKeeper<C>
where
    C: Consensus,
{
    height: C::Height,
    total_weight: Weight,
    rounds: BTreeMap<Round, RoundVotes<C>>,
}

impl<C> VoteKeeper<C>
where
    C: Consensus,
{
    pub fn new(height: C::Height, round: Round, total_weight: Weight) -> Self {
        let mut rounds = BTreeMap::new();

        rounds.insert(round, RoundVotes::new(height.clone(), round, total_weight));

        VoteKeeper {
            height,
            total_weight,
            rounds,
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(&mut self, vote: C::Vote, weight: Weight) -> Option<Event<C>> {
        let round = self.rounds.entry(vote.round()).or_insert_with(|| {
            RoundVotes::new(self.height.clone(), vote.round(), self.total_weight)
        });

        let vote_type = vote.vote_type();
        let threshold = round.add_vote(vote, weight);

        Self::to_event(vote_type, threshold)
    }

    /// Check if a threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        round: &Round,
        vote_type: VoteType,
        threshold: Threshold<ValueId<C>>,
    ) -> bool {
        let round = match self.rounds.get(round) {
            Some(round) => round,
            None => return false,
        };

        match vote_type {
            VoteType::Prevote => round.prevotes.is_threshold_met(threshold),
            VoteType::Precommit => round.precommits.is_threshold_met(threshold),
        }
    }

    /// Map a vote type and a threshold to a state machine event.
    fn to_event(typ: VoteType, threshold: Threshold<ValueId<C>>) -> Option<Event<C>> {
        match (typ, threshold) {
            (_, Threshold::Init) => None,

            (VoteType::Prevote, Threshold::Any) => Some(Event::PolkaAny),
            (VoteType::Prevote, Threshold::Nil) => Some(Event::PolkaNil),
            (VoteType::Prevote, Threshold::Value(v)) => Some(Event::PolkaValue(v)),

            (VoteType::Precommit, Threshold::Any) => Some(Event::PrecommitAny),
            (VoteType::Precommit, Threshold::Nil) => None,
            (VoteType::Precommit, Threshold::Value(v)) => Some(Event::PrecommitValue(v)),
        }
    }
}

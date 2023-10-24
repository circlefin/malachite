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
    pub fn new(height: Height, round: Round, total_weight: Weight) -> Self {
        let mut rounds = BTreeMap::new();

        rounds.insert(round, RoundVotes::new(height, round, total_weight));

        VoteKeeper {
            height,
            total_weight,
            rounds,
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(&mut self, vote: Vote, weight: Weight) -> Option<Event> {
        let round = self
            .rounds
            .entry(vote.round)
            .or_insert_with(|| RoundVotes::new(self.height, vote.round, self.total_weight));

        let vote_type = vote.typ;
        let threshold = round.add_vote(vote, weight);

        Self::to_event(vote_type, threshold)
    }

    pub fn check_threshold(
        &self,
        round: &Round,
        vote_type: VoteType,
        threshold: Threshold,
    ) -> bool {
        let round = match self.rounds.get(round) {
            Some(round) => round,
            None => return false,
        };

        match vote_type {
            VoteType::Prevote => round.prevotes.check_threshold(threshold),
            VoteType::Precommit => round.precommits.check_threshold(threshold),
        }
    }

    /// Map a vote type and a threshold to a state machine event.
    fn to_event(typ: VoteType, threshold: Threshold) -> Option<Event> {
        match (typ, threshold) {
            (_, Threshold::Init) => None,

            (VoteType::Prevote, Threshold::Any) => Some(Event::PolkaAny),
            (VoteType::Prevote, Threshold::Nil) => Some(Event::PolkaNil),
            (VoteType::Prevote, Threshold::Value(v)) => Some(Event::PolkaValue(*v.as_ref())),

            (VoteType::Precommit, Threshold::Any) => Some(Event::PrecommitAny),
            (VoteType::Precommit, Threshold::Nil) => None,
            (VoteType::Precommit, Threshold::Value(v)) => Some(Event::PrecommitValue(*v.as_ref())),
        }
    }
}

#[cfg(test)]
mod tests {
    use malachite_common::{Address, ValueId};

    use super::*;

    #[test]
    fn prevote_apply_nil() {
        let mut keeper = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

        let vote = Vote::new_prevote(Round::new(0), None, Address::new(1));

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote, 1);
        assert_eq!(event, Some(Event::PolkaNil));
    }

    #[test]
    fn precommit_apply_nil() {
        let mut keeper = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

        let vote = Vote::new_precommit(Round::new(0), None, Address::new(1));

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote, 1);
        assert_eq!(event, None);
    }

    #[test]
    fn prevote_apply_single_value() {
        let mut keeper = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

        let v = ValueId::new(1);
        let val = Some(v);
        let vote = Vote::new_prevote(Round::new(0), val, Address::new(1));

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let vote_nil = Vote::new_prevote(Round::new(0), None, Address::new(2));
        let event = keeper.apply_vote(vote_nil, 1);
        assert_eq!(event, Some(Event::PolkaAny));

        let event = keeper.apply_vote(vote, 1);
        assert_eq!(event, Some(Event::PolkaValue(v)));
    }

    #[test]
    fn precommit_apply_single_value() {
        let mut keeper = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

        let v = ValueId::new(1);
        let val = Some(v);
        let vote = Vote::new_precommit(Round::new(0), val, Address::new(1));

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let event = keeper.apply_vote(vote.clone(), 1);
        assert_eq!(event, None);

        let vote_nil = Vote::new_precommit(Round::new(0), None, Address::new(2));
        let event = keeper.apply_vote(vote_nil, 1);
        assert_eq!(event, Some(Event::PrecommitAny));

        let event = keeper.apply_vote(vote, 1);
        assert_eq!(event, Some(Event::PrecommitValue(v)));
    }
}

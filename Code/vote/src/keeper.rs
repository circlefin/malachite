use alloc::collections::BTreeMap;

use malachite_common::{Context, Round, ValueId, Vote, VoteType};

use crate::{
    count::{Threshold, Weight},
    RoundVotes,
};

/// Messages emitted by the vote keeper
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<Value> {
    PolkaAny,
    PolkaNil,
    PolkaValue(Value),
    PrecommitAny,
    PrecommitValue(Value),
}

/// Keeps track of votes and emits messages when thresholds are reached.
#[derive(Clone, Debug)]
pub struct VoteKeeper<Ctx>
where
    Ctx: Context,
{
    height: Ctx::Height,
    total_weight: Weight,
    rounds: BTreeMap<Round, RoundVotes<Ctx>>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    pub fn new(height: Ctx::Height, round: Round, total_weight: Weight) -> Self {
        let mut rounds = BTreeMap::new();

        rounds.insert(round, RoundVotes::new(height.clone(), round, total_weight));

        VoteKeeper {
            height,
            total_weight,
            rounds,
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(&mut self, vote: Ctx::Vote, weight: Weight) -> Option<Message<ValueId<Ctx>>> {
        let round = self.rounds.entry(vote.round()).or_insert_with(|| {
            RoundVotes::new(self.height.clone(), vote.round(), self.total_weight)
        });

        let vote_type = vote.vote_type();
        let threshold = round.add_vote(vote, weight);

        Self::to_message(vote_type, threshold)
    }

    /// Check if a threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        round: &Round,
        vote_type: VoteType,
        threshold: Threshold<ValueId<Ctx>>,
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
    fn to_message(
        typ: VoteType,
        threshold: Threshold<ValueId<Ctx>>,
    ) -> Option<Message<ValueId<Ctx>>> {
        match (typ, threshold) {
            (_, Threshold::Init) => None,

            (VoteType::Prevote, Threshold::Any) => Some(Message::PolkaAny),
            (VoteType::Prevote, Threshold::Nil) => Some(Message::PolkaNil),
            (VoteType::Prevote, Threshold::Value(v)) => Some(Message::PolkaValue(v)),

            (VoteType::Precommit, Threshold::Any) => Some(Message::PrecommitAny),
            (VoteType::Precommit, Threshold::Nil) => None,
            (VoteType::Precommit, Threshold::Value(v)) => Some(Message::PrecommitValue(v)),
        }
    }
}

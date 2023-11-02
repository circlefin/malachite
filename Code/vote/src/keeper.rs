use alloc::collections::{BTreeMap, BTreeSet};

use malachite_common::{Context, Round, ValueId, Vote, VoteType};

use crate::round_votes::RoundVotes;
use crate::round_weights::RoundWeights;
use crate::{Threshold, Weight};

/// Messages emitted by the vote keeper
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<Value> {
    PolkaAny,
    PolkaNil,
    PolkaValue(Value),
    PrecommitAny,
    PrecommitValue(Value),
    SkipRound,
}

#[derive(Clone, Debug)]
struct PerRound<Ctx>
where
    Ctx: Context,
{
    votes: RoundVotes<Ctx::Address, ValueId<Ctx>>,
    addresses_weights: RoundWeights<Ctx::Address>,
    emitted_msgs: BTreeSet<Message<ValueId<Ctx>>>,
}

impl<Ctx> PerRound<Ctx>
where
    Ctx: Context,
{
    fn new(total_weight: Weight) -> Self {
        Self {
            votes: RoundVotes::new(total_weight),
            addresses_weights: RoundWeights::new(),
            emitted_msgs: BTreeSet::new(),
        }
    }
}

/// Keeps track of votes and emits messages when thresholds are reached.
#[derive(Clone, Debug)]
pub struct VoteKeeper<Ctx>
where
    Ctx: Context,
{
    height: Ctx::Height,
    total_weight: Weight,
    per_round: BTreeMap<Round, PerRound<Ctx>>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    pub fn new(height: Ctx::Height, total_weight: Weight) -> Self {
        VoteKeeper {
            height,
            total_weight,
            per_round: BTreeMap::new(),
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(&mut self, vote: Ctx::Vote, weight: Weight) -> Option<Message<ValueId<Ctx>>> {
        let round = self
            .per_round
            .entry(vote.round())
            .or_insert_with(|| PerRound::new(self.total_weight));

        let vote_type = vote.vote_type();

        let threshold = round.votes.add_vote(
            vote_type,
            vote.validator_address().clone(),
            vote.take_value(),
            weight,
        );

        Self::to_message(vote_type, threshold)
    }

    /// Check if a threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        round: &Round,
        vote_type: VoteType,
        threshold: Threshold<ValueId<Ctx>>,
    ) -> bool {
        self.per_round.get(round).map_or(false, |round| {
            round.votes.is_threshold_met(vote_type, threshold)
        })
    }

    /// Map a vote type and a threshold to a state machine event.
    fn to_message(
        typ: VoteType,
        threshold: Threshold<ValueId<Ctx>>,
    ) -> Option<Message<ValueId<Ctx>>> {
        match (typ, threshold) {
            (_, Threshold::Unreached) => None,
            (_, Threshold::Skip) => Some(Message::SkipRound),

            (VoteType::Prevote, Threshold::Any) => Some(Message::PolkaAny),
            (VoteType::Prevote, Threshold::Nil) => Some(Message::PolkaNil),
            (VoteType::Prevote, Threshold::Value(v)) => Some(Message::PolkaValue(v)),

            (VoteType::Precommit, Threshold::Any) => Some(Message::PrecommitAny),
            (VoteType::Precommit, Threshold::Nil) => Some(Message::PrecommitAny),
            (VoteType::Precommit, Threshold::Value(v)) => Some(Message::PrecommitValue(v)),
        }
    }
}

fn is_skip(weight: Weight, total: Weight) -> bool {
    3 * weight > total
}

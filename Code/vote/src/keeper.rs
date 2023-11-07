use alloc::collections::{BTreeMap, BTreeSet};

use malachite_common::{Context, Round, ValueId, Vote, VoteType};

use crate::round_votes::RoundVotes;
use crate::round_weights::RoundWeights;
use crate::{Threshold, ThresholdParam, ThresholdParams, Weight};

/// Messages emitted by the vote keeper
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Message<Value> {
    PolkaAny,
    PolkaNil,
    PolkaValue(Value),
    PrecommitAny,
    PrecommitValue(Value),
    SkipRound(Round),
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
    fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        Self {
            votes: RoundVotes::new(total_weight, threshold_params),
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
    threshold_params: ThresholdParams,
    total_weight: Weight,
    per_round: BTreeMap<Round, PerRound<Ctx>>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    pub fn new(total_weight: Weight) -> Self {
        VoteKeeper {
            // TODO: Make these configurable
            threshold_params: ThresholdParams::default(),

            total_weight,
            per_round: BTreeMap::new(),
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(&mut self, vote: Ctx::Vote, weight: Weight) -> Option<Message<ValueId<Ctx>>> {
        let round = self
            .per_round
            .entry(vote.round())
            .or_insert_with(|| PerRound::new(self.total_weight, self.threshold_params));

        let threshold = round.votes.add_vote(
            vote.vote_type(),
            vote.validator_address().clone(),
            vote.value().cloned(),
            weight,
        );

        round
            .addresses_weights
            .set_once(vote.validator_address().clone(), weight);

        let msg = threshold_to_message(vote.vote_type(), vote.round(), threshold)?;

        let final_msg = if !round.emitted_msgs.contains(&msg) {
            Some(msg)
        } else if Self::skip_round(round, self.total_weight, self.threshold_params.honest) {
            Some(Message::SkipRound(vote.round()))
        } else {
            None
        };

        if let Some(final_msg) = &final_msg {
            round.emitted_msgs.insert(final_msg.clone());
        }

        final_msg
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

    /// Check whether or not we should skip this round, in case we haven't emitted any messages
    /// yet, and we have reached an honest threshold for the round.
    fn skip_round(
        round: &PerRound<Ctx>,
        total_weight: Weight,
        threshold_param: ThresholdParam,
    ) -> bool {
        round.emitted_msgs.is_empty()
            && threshold_param.is_met(round.addresses_weights.sum(), total_weight)
    }
}

/// Map a vote type and a threshold to a state machine event.
fn threshold_to_message<Value>(
    typ: VoteType,
    round: Round,
    threshold: Threshold<Value>,
) -> Option<Message<Value>> {
    match (typ, threshold) {
        (_, Threshold::Unreached) => None,
        (_, Threshold::Skip) => Some(Message::SkipRound(round)),

        (VoteType::Prevote, Threshold::Any) => Some(Message::PolkaAny),
        (VoteType::Prevote, Threshold::Nil) => Some(Message::PolkaNil),
        (VoteType::Prevote, Threshold::Value(v)) => Some(Message::PolkaValue(v)),

        (VoteType::Precommit, Threshold::Any) => Some(Message::PrecommitAny),
        (VoteType::Precommit, Threshold::Nil) => Some(Message::PrecommitAny),
        (VoteType::Precommit, Threshold::Value(v)) => Some(Message::PrecommitValue(v)),
    }
}

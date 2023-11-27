use core::fmt;

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
    fn new() -> Self {
        Self {
            votes: RoundVotes::new(),
            addresses_weights: RoundWeights::new(),
            emitted_msgs: BTreeSet::new(),
        }
    }
}

impl<Ctx> Clone for PerRound<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        Self {
            votes: self.votes.clone(),
            addresses_weights: self.addresses_weights.clone(),
            emitted_msgs: self.emitted_msgs.clone(),
        }
    }
}

impl<Ctx> fmt::Debug for PerRound<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PerRound")
            .field("votes", &self.votes)
            .field("addresses_weights", &self.addresses_weights)
            .field("emitted_msgs", &self.emitted_msgs)
            .finish()
    }
}

/// Keeps track of votes and emits messages when thresholds are reached.
pub struct VoteKeeper<Ctx>
where
    Ctx: Context,
{
    total_weight: Weight,
    threshold_params: ThresholdParams,
    per_round: BTreeMap<Round, PerRound<Ctx>>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    pub fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        VoteKeeper {
            total_weight,
            threshold_params,
            per_round: BTreeMap::new(),
        }
    }

    /// Apply a vote with a given weight, potentially triggering an event.
    pub fn apply_vote(
        &mut self,
        vote: Ctx::Vote,
        weight: Weight,
        current_round: Round,
    ) -> Option<Message<ValueId<Ctx>>> {
        let round = self
            .per_round
            .entry(vote.round())
            .or_insert_with(PerRound::new);

        round.votes.add_vote(
            vote.vote_type(),
            vote.validator_address().clone(),
            vote.value().clone(),
            weight,
        );

        round
            .addresses_weights
            .set_once(vote.validator_address().clone(), weight);

        if vote.round() > current_round {
            let combined_weight = round.addresses_weights.sum();

            let skip_round = self
                .threshold_params
                .honest
                .is_met(combined_weight, self.total_weight);

            if skip_round {
                let msg = Message::SkipRound(vote.round());
                round.emitted_msgs.insert(msg.clone());
                return Some(msg);
            }
        }

        let threshold = compute_threshold(
            vote.vote_type(),
            round,
            vote.value(),
            self.threshold_params.quorum,
            self.total_weight,
        );

        let msg = threshold_to_message(vote.vote_type(), vote.round(), threshold);

        match msg {
            Some(msg) if !round.emitted_msgs.contains(&msg) => {
                round.emitted_msgs.insert(msg.clone());
                Some(msg)
            }
            _ => None,
        }
    }

    /// Check if a threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        round: &Round,
        vote_type: VoteType,
        threshold: Threshold<ValueId<Ctx>>,
    ) -> bool {
        self.per_round.get(round).map_or(false, |round| {
            round.votes.is_threshold_met(
                vote_type,
                threshold,
                self.threshold_params.quorum,
                self.total_weight,
            )
        })
    }
}

/// Compute whether or not we have reached a threshold for the given value,
/// and return that threshold.
fn compute_threshold<Ctx>(
    vote_type: VoteType,
    round: &PerRound<Ctx>,
    value: &Option<ValueId<Ctx>>,
    quorum: ThresholdParam,
    total_weight: Weight,
) -> Threshold<ValueId<Ctx>>
where
    Ctx: Context,
{
    let weight = round.votes.get_weight(vote_type, value);

    match value {
        Some(value) if quorum.is_met(weight, total_weight) => Threshold::Value(value.clone()),

        None if quorum.is_met(weight, total_weight) => Threshold::Nil,

        _ => {
            let weight_sum = round.votes.weight_sum(vote_type);

            if quorum.is_met(weight_sum, total_weight) {
                Threshold::Any
            } else {
                Threshold::Unreached
            }
        }
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

impl<Ctx> Clone for VoteKeeper<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        Self {
            total_weight: self.total_weight,
            threshold_params: self.threshold_params,
            per_round: self.per_round.clone(),
        }
    }
}

impl<Ctx> fmt::Debug for VoteKeeper<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoteKeeper")
            .field("total_weight", &self.total_weight)
            .field("threshold_params", &self.threshold_params)
            .field("per_round", &self.per_round)
            .finish()
    }
}

//! For tallying votes and emitting messages when certain thresholds are reached.

use core::fmt;

use alloc::collections::{BTreeMap, BTreeSet};

use malachite_common::{Context, NilOrVal, Round, ValueId, Vote, VoteType};

use crate::round_votes::RoundVotes;
use crate::round_weights::RoundWeights;
use crate::{Threshold, ThresholdParam, ThresholdParams, Weight};

/// Messages emitted by the vote keeper
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Output<Value> {
    /// We have a quorum of prevotes for some value or nil
    PolkaAny,

    /// We have a quorum of prevotes for nil
    PolkaNil,

    /// We have a quorum of prevotes for specific value
    PolkaValue(Value),

    /// We have a quorum of precommits for some value or nil
    PrecommitAny,

    /// We have a quorum of precommits for a specific value
    PrecommitValue(Value),

    /// We have f+1 honest votes for a value at a higher round
    SkipRound(Round),
}

/// Keeps track of votes and emitted outputs for a given round.
pub struct PerRound<Ctx>
where
    Ctx: Context,
{
    /// The votes for this round.
    votes: RoundVotes<Ctx::Address, ValueId<Ctx>>,
    /// The addresses and their weights for this round.
    addresses_weights: RoundWeights<Ctx::Address>,
    /// The emitted outputs for this round.
    emitted_outputs: BTreeSet<Output<ValueId<Ctx>>>,
}

impl<Ctx> PerRound<Ctx>
where
    Ctx: Context,
{
    /// Create a new `PerRound` instance.
    fn new() -> Self {
        Self {
            votes: RoundVotes::new(),
            addresses_weights: RoundWeights::new(),
            emitted_outputs: BTreeSet::new(),
        }
    }

    /// Return the votes for this round.
    pub fn votes(&self) -> &RoundVotes<Ctx::Address, ValueId<Ctx>> {
        &self.votes
    }

    /// Return the addresses and their weights for this round.
    pub fn addresses_weights(&self) -> &RoundWeights<Ctx::Address> {
        &self.addresses_weights
    }

    /// Return the emitted outputs for this round.
    pub fn emitted_outputs(&self) -> &BTreeSet<Output<ValueId<Ctx>>> {
        &self.emitted_outputs
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
            emitted_outputs: self.emitted_outputs.clone(),
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
            .field("emitted_outputs", &self.emitted_outputs)
            .finish()
    }
}

/// Keeps track of votes and emits messages when thresholds are reached.
pub struct VoteKeeper<Ctx>
where
    Ctx: Context,
{
    /// The total weight (ie. voting power) of the network.
    total_weight: Weight,
    /// The threshold parameters.
    threshold_params: ThresholdParams,
    /// The votes and emitted outputs for each round.
    per_round: BTreeMap<Round, PerRound<Ctx>>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    /// Create a new `VoteKeeper` instance, for the given
    /// total network weight (ie. voting power) and threshold parameters.
    pub fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        VoteKeeper {
            total_weight,
            threshold_params,
            per_round: BTreeMap::new(),
        }
    }

    /// Return the total weight (ie. voting power) of the network.
    pub fn total_weight(&self) -> &Weight {
        &self.total_weight
    }

    /// Return the threshold parameters.
    pub fn per_round(&self) -> &BTreeMap<Round, PerRound<Ctx>> {
        &self.per_round
    }

    /// Apply a vote with a given weight, potentially triggering an output.
    pub fn apply_vote(
        &mut self,
        vote: Ctx::Vote,
        weight: Weight,
        current_round: Round,
    ) -> Option<Output<ValueId<Ctx>>> {
        let per_round = self
            .per_round
            .entry(vote.round())
            .or_insert_with(PerRound::new);

        per_round.votes.add_vote(
            vote.vote_type(),
            vote.validator_address().clone(),
            vote.value().clone(),
            weight,
        );

        per_round
            .addresses_weights
            .set_once(vote.validator_address().clone(), weight);

        if vote.round() > current_round {
            let combined_weight = per_round.addresses_weights.sum();

            let skip_round = self
                .threshold_params
                .honest
                .is_met(combined_weight, self.total_weight);

            if skip_round {
                let output = Output::SkipRound(vote.round());
                per_round.emitted_outputs.insert(output.clone());
                return Some(output);
            }
        }

        let threshold = compute_threshold(
            vote.vote_type(),
            per_round,
            vote.value(),
            self.threshold_params.quorum,
            self.total_weight,
        );

        let output = threshold_to_output(vote.vote_type(), vote.round(), threshold);

        match output {
            Some(output) if !per_round.emitted_outputs.contains(&output) => {
                per_round.emitted_outputs.insert(output.clone());
                Some(output)
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
        self.per_round.get(round).map_or(false, |per_round| {
            per_round.votes.is_threshold_met(
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
    value: &NilOrVal<ValueId<Ctx>>,
    quorum: ThresholdParam,
    total_weight: Weight,
) -> Threshold<ValueId<Ctx>>
where
    Ctx: Context,
{
    let weight = round.votes.get_weight(vote_type, value);

    match value {
        NilOrVal::Val(value) if quorum.is_met(weight, total_weight) => {
            Threshold::Value(value.clone())
        }

        NilOrVal::Nil if quorum.is_met(weight, total_weight) => Threshold::Nil,

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

/// Map a vote type and a threshold to a state machine output.
fn threshold_to_output<Value>(
    typ: VoteType,
    round: Round,
    threshold: Threshold<Value>,
) -> Option<Output<Value>> {
    match (typ, threshold) {
        (_, Threshold::Unreached) => None,
        (_, Threshold::Skip) => Some(Output::SkipRound(round)),

        (VoteType::Prevote, Threshold::Any) => Some(Output::PolkaAny),
        (VoteType::Prevote, Threshold::Nil) => Some(Output::PolkaNil),
        (VoteType::Prevote, Threshold::Value(v)) => Some(Output::PolkaValue(v)),

        (VoteType::Precommit, Threshold::Any) => Some(Output::PrecommitAny),
        (VoteType::Precommit, Threshold::Nil) => Some(Output::PrecommitAny),
        (VoteType::Precommit, Threshold::Value(v)) => Some(Output::PrecommitValue(v)),
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

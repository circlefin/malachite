//! For tallying votes and emitting messages when certain thresholds are reached.

use derive_where::derive_where;

use alloc::collections::{BTreeMap, BTreeSet};

use malachite_common::{Context, NilOrVal, Round, ValueId, Vote, VoteType};

use crate::evidence::EvidenceMap;
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
#[derive_where(Clone, Debug, PartialEq, Eq, Default)]
pub struct PerRound<Ctx>
where
    Ctx: Context,
{
    /// The votes for this round.
    votes: RoundVotes<Ctx::Address, ValueId<Ctx>>,

    /// The addresses and their weights for this round.
    addresses_weights: RoundWeights<Ctx::Address>,

    /// All the votes received for this round.
    received_votes: BTreeSet<Ctx::Vote>,

    /// The emitted outputs for this round.
    emitted_outputs: BTreeSet<Output<ValueId<Ctx>>>,
}

/// Errors can that be yielded when recording a vote.
pub enum RecordVoteError<Ctx>
where
    Ctx: Context,
{
    /// Attempted to record a conflicting vote.
    ConflictingVote {
        /// The vote already recorded.
        existing: Ctx::Vote,
        /// The conflicting vote.
        conflicting: Ctx::Vote,
    },
}

impl<Ctx> PerRound<Ctx>
where
    Ctx: Context,
{
    /// Create a new `PerRound` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a vote to the round, checking for conflicts.
    pub fn add(&mut self, vote: Ctx::Vote, weight: Weight) -> Result<(), RecordVoteError<Ctx>> {
        if let Some(existing) = self.get_vote(vote.vote_type(), vote.validator_address()) {
            if existing.value() != vote.value() {
                // This is an equivocating vote
                return Err(RecordVoteError::ConflictingVote {
                    existing: existing.clone(),
                    conflicting: vote,
                });
            }
        }

        // Add the vote to the round
        self.votes.add_vote(
            vote.vote_type(),
            vote.validator_address().clone(),
            vote.value().clone(),
            weight,
        );

        // Update the weight of the validator
        self.addresses_weights
            .set_once(vote.validator_address(), weight);

        // Add the vote to the received votes
        self.received_votes.insert(vote);

        Ok(())
    }

    /// Return the vote of the given type received from the given validator.
    pub fn get_vote<'a>(
        &'a self,
        vote_type: VoteType,
        address: &'a Ctx::Address,
    ) -> Option<&'a Ctx::Vote> {
        self.received_votes
            .iter()
            .find(move |vote| vote.vote_type() == vote_type && vote.validator_address() == address)
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

/// Keeps track of votes and emits messages when thresholds are reached.
#[derive_where(Clone, Debug)]
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
    /// Evidence of equivocation.
    evidence: EvidenceMap<Ctx>,
}

impl<Ctx> VoteKeeper<Ctx>
where
    Ctx: Context,
{
    /// Create a new `VoteKeeper` instance, for the given
    /// total network weight (ie. voting power) and threshold parameters.
    pub fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        Self {
            total_weight,
            threshold_params,
            per_round: BTreeMap::new(),
            evidence: EvidenceMap::new(),
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

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Apply a vote with a given weight, potentially triggering an output.
    pub fn apply_vote(
        &mut self,
        vote: Ctx::Vote,
        weight: Weight,
        current_round: Round,
    ) -> Option<Output<ValueId<Ctx>>> {
        let per_round = self.per_round.entry(vote.round()).or_default();

        match per_round.add(vote.clone(), weight) {
            Ok(()) => (),
            Err(RecordVoteError::ConflictingVote {
                existing,
                conflicting: vote,
            }) => {
                // This is an equivocating vote
                self.evidence.add(existing, vote);

                return None;
            }
        }

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

        let output = threshold_to_output(vote.vote_type(), threshold);

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
fn threshold_to_output<Value>(typ: VoteType, threshold: Threshold<Value>) -> Option<Output<Value>> {
    match (typ, threshold) {
        (_, Threshold::Unreached) => None,

        (VoteType::Prevote, Threshold::Any) => Some(Output::PolkaAny),
        (VoteType::Prevote, Threshold::Nil) => Some(Output::PolkaNil),
        (VoteType::Prevote, Threshold::Value(v)) => Some(Output::PolkaValue(v)),

        (VoteType::Precommit, Threshold::Any) => Some(Output::PrecommitAny),
        (VoteType::Precommit, Threshold::Nil) => Some(Output::PrecommitAny),
        (VoteType::Precommit, Threshold::Value(v)) => Some(Output::PrecommitValue(v)),
    }
}

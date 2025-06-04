//! For tallying votes and emitting messages when certain thresholds are reached.

use derive_where::derive_where;
use thiserror::Error;

use alloc::collections::{BTreeMap, BTreeSet};

use malachitebft_core_types::{
    Context, NilOrVal, Round, SignedVote, Validator, ValidatorSet, ValueId, Vote, VoteType,
};

use crate::evidence::EvidenceMap;
use crate::round_votes::RoundVotes;
use crate::round_weights::RoundWeights;
use crate::{Threshold, ThresholdParams, Weight};

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
    votes: RoundVotes<Ctx>,

    /// The addresses and their weights for this round.
    addresses_weights: RoundWeights<Ctx::Address>,

    /// All the votes received for this round.
    received_votes: BTreeSet<SignedVote<Ctx>>,

    /// The emitted outputs for this round.
    emitted_outputs: BTreeSet<Output<ValueId<Ctx>>>,
}

/// Errors can that be yielded when recording a vote.
#[derive(Error)]
pub enum RecordVoteError<Ctx>
where
    Ctx: Context,
{
    /// Attempted to record a conflicting vote.
    #[error("Conflicting vote: {existing} vs {conflicting}")]
    ConflictingVote {
        /// The vote already recorded.
        existing: SignedVote<Ctx>,
        /// The conflicting vote.
        conflicting: SignedVote<Ctx>,
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
    pub fn add(
        &mut self,
        vote: SignedVote<Ctx>,
        weight: Weight,
    ) -> Result<(), RecordVoteError<Ctx>> {
        if let Some(existing) = self.get_vote(vote.vote_type(), vote.validator_address()) {
            if existing.value() != vote.value() {
                // This is an equivocating vote
                return Err(RecordVoteError::ConflictingVote {
                    existing: existing.clone(),
                    conflicting: vote,
                });
            }
        }

        // Tally this vote
        self.votes.add_vote(&vote, weight);

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
    ) -> Option<&'a SignedVote<Ctx>> {
        self.received_votes
            .iter()
            .find(move |vote| vote.vote_type() == vote_type && vote.validator_address() == address)
    }

    /// Return the votes for this round.
    pub fn votes(&self) -> &RoundVotes<Ctx> {
        &self.votes
    }

    /// Return the votes for this round.
    pub fn received_votes(&self) -> &BTreeSet<SignedVote<Ctx>> {
        &self.received_votes
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
    /// The validator set for this height.
    validator_set: Ctx::ValidatorSet,

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
    pub fn new(validator_set: Ctx::ValidatorSet, threshold_params: ThresholdParams) -> Self {
        Self {
            validator_set,
            threshold_params,
            per_round: BTreeMap::new(),
            evidence: EvidenceMap::new(),
        }
    }

    /// Return the current validator set
    pub fn validator_set(&self) -> &Ctx::ValidatorSet {
        &self.validator_set
    }

    /// Return the total weight (ie. voting power) of the network.
    pub fn total_weight(&self) -> Weight {
        self.validator_set.total_voting_power()
    }

    /// Return the votes for the given round.
    pub fn per_round(&self, round: Round) -> Option<&PerRound<Ctx>> {
        self.per_round.get(&round)
    }

    /// Return how many rounds we have seen votes for so far.
    pub fn rounds(&self) -> usize {
        self.per_round.len()
    }

    /// Return the highest round we have seen votes for so far.
    pub fn max_round(&self) -> Round {
        self.per_round.keys().max().copied().unwrap_or(Round::Nil)
    }

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Check if we have already seen a vote.
    pub fn has_vote(&self, vote: &SignedVote<Ctx>) -> bool {
        self.per_round
            .get(&vote.round())
            .is_some_and(|per_round| per_round.received_votes().contains(vote))
    }

    /// Apply a vote with a given weight, potentially triggering an output.
    pub fn apply_vote(
        &mut self,
        vote: SignedVote<Ctx>,
        round: Round,
    ) -> Option<Output<ValueId<Ctx>>> {
        let total_weight = self.total_weight();
        let per_round = self.per_round.entry(vote.round()).or_default();

        let Some(validator) = self.validator_set.get_by_address(vote.validator_address()) else {
            // Vote from unknown validator, let's discard it.
            return None;
        };

        match per_round.add(vote.clone(), validator.voting_power()) {
            Ok(()) => (),
            Err(RecordVoteError::ConflictingVote {
                existing,
                conflicting,
            }) => {
                // This is an equivocating vote
                self.evidence.add(existing.clone(), conflicting);
                //panic!("Equivocating vote {:?}, existing {:?}", &vote, &existing);
                return None;
            }
        }

        if vote.round() > round {
            let combined_weight = per_round.addresses_weights.sum();

            let skip_round = self
                .threshold_params
                .honest
                .is_met(combined_weight, total_weight);

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
            self.threshold_params,
            total_weight,
        );

        let output = threshold_to_output(vote.vote_type(), threshold);

        match output {
            // Ensure we do not output the same message twice
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
        self.per_round.get(round).is_some_and(|per_round| {
            per_round.votes.is_threshold_met(
                vote_type,
                threshold,
                self.threshold_params.quorum,
                self.total_weight(),
            )
        })
    }

    /// Prunes all stored votes from rounds less than `min_round`.
    pub fn prune_votes(&mut self, min_round: Round) {
        self.per_round.retain(|round, _| *round >= min_round);
    }
}

/// Compute whether or not we have reached a threshold for the given value,
/// and return that threshold.
fn compute_threshold<Ctx>(
    vote_type: VoteType,
    round: &PerRound<Ctx>,
    value: &NilOrVal<ValueId<Ctx>>,
    thresholds: ThresholdParams,
    total_weight: Weight,
) -> Threshold<ValueId<Ctx>>
where
    Ctx: Context,
{
    let weight = round.votes.get_weight(vote_type, value);

    match value {
        NilOrVal::Val(value) if thresholds.quorum.is_met(weight, total_weight) => {
            Threshold::Value(value.clone())
        }

        NilOrVal::Nil if thresholds.quorum.is_met(weight, total_weight) => Threshold::Nil,

        _ => {
            let weight_sum = round.votes.weight_sum(vote_type);

            if thresholds.quorum.is_met(weight_sum, total_weight) {
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

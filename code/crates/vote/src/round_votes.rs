//! For tallying all the votes for a single round

use malachite_common::{NilOrVal, VoteType};

use crate::count::VoteCount;
use crate::{Threshold, ThresholdParam, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundVotes<Address, Value> {
    /// The prevotes for this round.
    prevotes: VoteCount<Address, Value>,
    /// The precommits for this round.
    precommits: VoteCount<Address, Value>,
}

impl<Address, Value> RoundVotes<Address, Value> {
    /// Create a new `RoundVotes` instance.
    pub fn new() -> Self {
        RoundVotes {
            prevotes: VoteCount::new(),
            precommits: VoteCount::new(),
        }
    }

    /// Return the prevotes for this round.
    pub fn prevotes(&self) -> &VoteCount<Address, Value> {
        &self.prevotes
    }

    /// Return the precommits for this round.
    pub fn precommits(&self) -> &VoteCount<Address, Value> {
        &self.precommits
    }

    /// Add a vote to the round, of the given type, from the given address,
    /// with the given value and weight.
    pub fn add_vote(
        &mut self,
        vote_type: VoteType,
        address: Address,
        value: NilOrVal<Value>,
        weight: Weight,
    ) -> Weight
    where
        Address: Clone + Ord,
        Value: Clone + Ord,
    {
        match vote_type {
            VoteType::Prevote => self.prevotes.add(address, value, weight),
            VoteType::Precommit => self.precommits.add(address, value, weight),
        }
    }

    /// Get the weight of the vote of the given type for the given value.
    ///
    /// If there is no vote for that value, return 0.
    pub fn get_weight(&self, vote_type: VoteType, value: &NilOrVal<Value>) -> Weight
    where
        Value: Ord,
    {
        match vote_type {
            VoteType::Prevote => self.prevotes.get(value),
            VoteType::Precommit => self.precommits.get(value),
        }
    }

    /// Get the sum of the weights of the votes of the given type.
    pub fn weight_sum(&self, vote_type: VoteType) -> Weight {
        match vote_type {
            VoteType::Prevote => self.prevotes.sum(),
            VoteType::Precommit => self.precommits.sum(),
        }
    }

    /// Get the sum of the weights of all votes, regardless of type, for the given value.
    pub fn combined_weight(&self, value: &NilOrVal<Value>) -> Weight
    where
        Value: Ord,
    {
        self.prevotes.get(value) + self.precommits.get(value)
    }

    /// Return whether or not the threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        vote_type: VoteType,
        threshold: Threshold<Value>,
        param: ThresholdParam,
        total_weight: Weight,
    ) -> bool
    where
        Value: Ord,
    {
        match vote_type {
            VoteType::Prevote => self
                .prevotes
                .is_threshold_met(threshold, param, total_weight),

            VoteType::Precommit => self
                .precommits
                .is_threshold_met(threshold, param, total_weight),
        }
    }
}

impl<Address, Value> Default for RoundVotes<Address, Value> {
    fn default() -> Self {
        Self::new()
    }
}

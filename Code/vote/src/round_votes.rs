use malachite_common::VoteType;

use crate::count::VoteCount;
use crate::{Threshold, ThresholdParam, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
pub struct RoundVotes<Address, Value> {
    prevotes: VoteCount<Address, Value>,
    precommits: VoteCount<Address, Value>,
}

impl<Address, Value> RoundVotes<Address, Value> {
    pub fn new() -> Self {
        RoundVotes {
            prevotes: VoteCount::new(),
            precommits: VoteCount::new(),
        }
    }

    pub fn add_vote(
        &mut self,
        vote_type: VoteType,
        address: Address,
        value: Option<Value>,
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

    pub fn get_weight(&self, vote_type: VoteType, value: &Option<Value>) -> Weight
    where
        Value: Ord,
    {
        match vote_type {
            VoteType::Prevote => self.prevotes.get(value),
            VoteType::Precommit => self.precommits.get(value),
        }
    }

    pub fn weight_sum(&self, vote_type: VoteType) -> Weight {
        match vote_type {
            VoteType::Prevote => self.prevotes.sum(),
            VoteType::Precommit => self.precommits.sum(),
        }
    }

    pub fn combined_weight(&self, value: &Option<Value>) -> Weight
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

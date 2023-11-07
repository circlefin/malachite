use malachite_common::VoteType;

use crate::count::VoteCount;
use crate::{Threshold, ThresholdParams, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
pub struct RoundVotes<Address, Value> {
    prevotes: VoteCount<Address, Value>,
    precommits: VoteCount<Address, Value>,
}

impl<Address, Value> RoundVotes<Address, Value> {
    pub fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        RoundVotes {
            prevotes: VoteCount::new(total_weight, threshold_params),
            precommits: VoteCount::new(total_weight, threshold_params),
        }
    }

    pub fn add_vote(
        &mut self,
        vote_type: VoteType,
        address: Address,
        value: Option<Value>,
        weight: Weight,
    ) -> Threshold<Value>
    where
        Address: Clone + Ord,
        Value: Clone + Ord,
    {
        match vote_type {
            VoteType::Prevote => self.prevotes.add(address, value, weight),
            VoteType::Precommit => self.precommits.add(address, value, weight),
        }
    }

    pub fn is_threshold_met(&self, vote_type: VoteType, threshold: Threshold<Value>) -> bool
    where
        Value: Ord,
    {
        match vote_type {
            VoteType::Prevote => self.prevotes.is_threshold_met(threshold),
            VoteType::Precommit => self.precommits.is_threshold_met(threshold),
        }
    }
}

use alloc::collections::BTreeSet;

use crate::value_weights::ValuesWeights;
use crate::{Threshold, ThresholdParams, Weight};

/// VoteCount tallys votes of the same type.
/// Votes are for nil or for some value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteCount<Address, Value> {
    /// Total weight
    pub total_weight: Weight,

    /// The threshold parameters
    pub threshold_params: ThresholdParams,

    /// Weight of votes for the values, including nil
    pub values_weights: ValuesWeights<Option<Value>>,

    /// Addresses of validators who voted for the values
    pub validator_addresses: BTreeSet<Address>,
}

impl<Address, Value> VoteCount<Address, Value> {
    pub fn new(total_weight: Weight, threshold_params: ThresholdParams) -> Self {
        VoteCount {
            total_weight,
            threshold_params,
            values_weights: ValuesWeights::new(),
            validator_addresses: BTreeSet::new(),
        }
    }

    /// Add vote for a value (or nil) to internal counters, but only if we haven't seen
    /// a vote from that particular validator yet.
    pub fn add(
        &mut self,
        address: Address,
        value: Option<Value>,
        weight: Weight,
    ) -> Threshold<Value>
    where
        Address: Clone + Ord,
        Value: Clone + Ord,
    {
        let already_voted = !self.validator_addresses.insert(address);

        if !already_voted {
            self.values_weights.add(value.clone(), weight);
        }

        self.compute_threshold(value)
    }

    /// Compute whether or not we have reached a threshold for the given value,
    /// and return that threshold.
    pub fn compute_threshold(&self, value: Option<Value>) -> Threshold<Value>
    where
        Address: Ord,
        Value: Ord,
    {
        let weight = self.values_weights.get(&value);

        match value {
            Some(value) if self.is_quorum(weight, self.total_weight) => Threshold::Value(value),

            None if self.is_quorum(weight, self.total_weight) => Threshold::Nil,

            _ => {
                let sum_weight = self.values_weights.sum();

                if self.is_quorum(sum_weight, self.total_weight) {
                    Threshold::Any
                } else {
                    Threshold::Unreached
                }
            }
        }
    }

    /// Return whether or not the threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(&self, threshold: Threshold<Value>) -> bool
    where
        Value: Ord,
    {
        match threshold {
            Threshold::Value(value) => {
                let weight = self.values_weights.get(&Some(value));
                self.is_quorum(weight, self.total_weight)
            }

            Threshold::Nil => {
                let weight = self.values_weights.get(&None);
                self.is_quorum(weight, self.total_weight)
            }

            Threshold::Any => {
                let sum_weight = self.values_weights.sum();
                self.is_quorum(sum_weight, self.total_weight)
            }

            Threshold::Skip | Threshold::Unreached => false,
        }
    }

    pub fn get(&self, value: &Option<Value>) -> Weight
    where
        Value: Ord,
    {
        self.values_weights.get(value)
    }

    pub fn total_weight(&self) -> Weight {
        self.total_weight
    }

    fn is_quorum(&self, sum: Weight, total: Weight) -> bool {
        self.threshold_params.quorum.is_met(sum, total)
    }
}

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;

    #[test]
    fn vote_count_nil() {
        let mut vc = VoteCount::new(4, Default::default());

        let addr1 = [1];
        let addr2 = [2];
        let addr3 = [3];
        let addr4 = [4];

        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 1);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr2, None, 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 2);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        // addr1 votes again, is ignored
        assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 2);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr3, None, 1), Threshold::Nil);
        assert_eq!(vc.get(&None), 3);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr4, Some(1), 1), Threshold::Any);
        assert_eq!(vc.get(&None), 3);
        assert_eq!(vc.get(&Some(1)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
    }

    #[test]
    fn vote_count_value() {
        let mut vc = VoteCount::new(4, Default::default());

        let addr1 = [1];
        let addr2 = [2];
        let addr3 = [3];
        let addr4 = [4];

        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr1, Some(1), 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr2, Some(1), 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 2);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        // addr1 votes again, for nil this time, is ignored
        assert_eq!(vc.add(addr1, None, 1), Threshold::Unreached);
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 2);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr3, Some(1), 1), Threshold::Value(1));
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 3);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        // addr2 votes again, for the same value, is ignored
        assert_eq!(vc.add(addr2, Some(1), 1), Threshold::Value(1));
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 3);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add(addr4, Some(2), 1), Threshold::Any);
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 3);
        assert_eq!(vc.get(&Some(2)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        // addr4 votes again, for a different value, is ignored
        assert_eq!(vc.add(addr4, Some(3), 1), Threshold::Any);
        assert_eq!(vc.get(&None), 0);
        assert_eq!(vc.get(&Some(1)), 3);
        assert_eq!(vc.get(&Some(2)), 1);
        assert_eq!(vc.get(&Some(3)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
    }
}

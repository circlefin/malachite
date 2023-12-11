//! For tallying votes of the same type.

use alloc::collections::BTreeSet;
use malachite_common::NilOrVal;

use crate::value_weights::ValuesWeights;
use crate::{Threshold, ThresholdParam, Weight};

/// VoteCount tallys votes of the same type.
///
/// Votes are for nil or for some value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteCount<Address, Value> {
    /// Weight of votes for the values, including nil
    pub values_weights: ValuesWeights<NilOrVal<Value>>,

    /// Addresses of validators who voted for the values
    pub validator_addresses: BTreeSet<Address>,
}

impl<Address, Value> VoteCount<Address, Value> {
    /// Create a new `VoteCount`.
    pub fn new() -> Self {
        VoteCount {
            values_weights: ValuesWeights::new(),
            validator_addresses: BTreeSet::new(),
        }
    }

    /// Add vote for a value (or nil) to internal counters, but only if we haven't seen
    /// a vote from that particular validator yet.
    pub fn add(&mut self, address: Address, value: NilOrVal<Value>, weight: Weight) -> Weight
    where
        Address: Clone + Ord,
        Value: Clone + Ord,
    {
        let already_voted = !self.validator_addresses.insert(address);

        if !already_voted {
            self.values_weights.add(value, weight)
        } else {
            self.values_weights.get(&value)
        }
    }

    /// Return the weight of votes for the given value (or nil).
    pub fn get(&self, value: &NilOrVal<Value>) -> Weight
    where
        Value: Ord,
    {
        self.values_weights.get(value)
    }

    /// Return the sum of weights of votes for all values.
    pub fn sum(&self) -> Weight {
        self.values_weights.sum()
    }

    /// Return whether or not the threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(
        &self,
        threshold: Threshold<Value>,
        param: ThresholdParam,
        total_weight: Weight,
    ) -> bool
    where
        Value: Ord,
    {
        match threshold {
            Threshold::Value(value) => {
                let weight = self.values_weights.get(&NilOrVal::Val(value));
                param.is_met(weight, total_weight)
            }

            Threshold::Nil => {
                let weight = self.values_weights.get(&NilOrVal::Nil);
                param.is_met(weight, total_weight)
            }

            Threshold::Any => {
                let sum_weight = self.values_weights.sum();
                param.is_met(sum_weight, total_weight)
            }

            Threshold::Skip | Threshold::Unreached => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;

    #[test]
    fn vote_count_nil() {
        let t = 4;
        let q = ThresholdParam::TWO_F_PLUS_ONE;

        let mut vc = VoteCount::new();

        let addr1 = [1];
        let addr2 = [2];
        let addr3 = [3];
        let addr4 = [4];

        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr1, NilOrVal::Nil, 1), 1);
        assert_eq!(vc.get(&NilOrVal::Nil), 1);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr2, NilOrVal::Nil, 1), 2);
        assert_eq!(vc.get(&NilOrVal::Nil), 2);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        // addr1 votes again, is ignored
        assert_eq!(vc.add(addr1, NilOrVal::Nil, 1), 2);
        assert_eq!(vc.get(&NilOrVal::Nil), 2);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr3, NilOrVal::Nil, 1), 3);
        assert_eq!(vc.get(&NilOrVal::Nil), 3);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr4, NilOrVal::Val(1), 1), 1);
        assert_eq!(vc.get(&NilOrVal::Nil), 3);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);
    }

    #[test]
    fn vote_count_value() {
        let t = 4;
        let q = ThresholdParam::TWO_F_PLUS_ONE;

        let mut vc = VoteCount::new();

        let addr1 = [1];
        let addr2 = [2];
        let addr3 = [3];
        let addr4 = [4];

        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr1, NilOrVal::Val(1), 1), 1);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr2, NilOrVal::Val(1), 1), 2);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 2);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        // addr1 votes again, for nil this time, is ignored
        assert_eq!(vc.add(addr1, NilOrVal::Nil, 1), 0);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 2);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr3, NilOrVal::Val(1), 1), 3);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 3);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        // addr2 votes again, for the same value, is ignored
        assert_eq!(vc.add(addr2, NilOrVal::Val(1), 1), 3);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 3);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        assert_eq!(vc.add(addr4, NilOrVal::Val(2), 1), 1);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 3);
        assert_eq!(vc.get(&NilOrVal::Val(2)), 1);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);

        // addr4 votes again, for a different value, is ignored
        assert_eq!(vc.add(addr4, NilOrVal::Val(3), 1), 0);
        assert_eq!(vc.get(&NilOrVal::Nil), 0);
        assert_eq!(vc.get(&NilOrVal::Val(1)), 3);
        assert_eq!(vc.get(&NilOrVal::Val(2)), 1);
        assert_eq!(vc.get(&NilOrVal::Val(3)), 0);
        assert_eq!(vc.is_threshold_met(Threshold::Unreached, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any, q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil, q, t), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1), q, t), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2), q, t), false);
    }
}

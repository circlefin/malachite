use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;

// TODO: Introduce newtype
// QUESTION: Over what type? i64?
pub type Weight = u64;

/// A value and the weight of votes for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuesWeights<Value> {
    value_weights: BTreeMap<Value, Weight>,
}

impl<Value> ValuesWeights<Value> {
    pub fn new() -> ValuesWeights<Value> {
        ValuesWeights {
            value_weights: BTreeMap::new(),
        }
    }

    /// Add weight to the value and return the new weight.
    pub fn add(&mut self, value: Value, weight: Weight) -> Weight
    where
        Value: Ord,
    {
        let entry = self.value_weights.entry(value).or_insert(0);
        *entry += weight; // FIXME: Deal with overflows
        *entry
    }

    /// Return the weight of the value, or 0 if it is not present.
    pub fn get(&self, value: &Value) -> Weight
    where
        Value: Ord,
    {
        self.value_weights.get(value).copied().unwrap_or(0)
    }

    /// Return the sum of the weights of all values.
    pub fn sum(&self) -> Weight {
        self.value_weights.values().sum() // FIXME: Deal with overflows
    }
}

impl<Value> Default for ValuesWeights<Value> {
    fn default() -> Self {
        Self::new()
    }
}

/// VoteCount tallys votes of the same type.
/// Votes are for nil or for some value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteCount<Address, Value> {
    /// Total weight
    pub total_weight: Weight,

    /// Weight of votes for the values, including nil
    pub values_weights: ValuesWeights<Option<Value>>,

    /// Addresses of validators who voted for the values
    pub validator_addresses: BTreeSet<Address>,
}

impl<Address, Value> VoteCount<Address, Value> {
    pub fn new(total_weight: Weight) -> Self {
        VoteCount {
            total_weight,
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
            Some(value) if is_quorum(weight, self.total_weight) => Threshold::Value(value),

            None if is_quorum(weight, self.total_weight) => Threshold::Nil,

            _ => {
                let sum_weight = self.values_weights.sum();

                if is_quorum(sum_weight, self.total_weight) {
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
                is_quorum(weight, self.total_weight)
            }

            Threshold::Nil => {
                let weight = self.values_weights.get(&None);
                is_quorum(weight, self.total_weight)
            }

            Threshold::Any => {
                let sum_weight = self.values_weights.sum();
                is_quorum(sum_weight, self.total_weight)
            }

            Threshold::Unreached => false,
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
}

//-------------------------------------------------------------------------
// Round votes
//-------------------------------------------------------------------------

// Thresh represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Threshold<ValueId> {
    /// No quorum has been reached yet
    Unreached,
    /// Qorum of votes but not for the same value
    Any,
    /// Quorum for nil
    Nil,
    /// Quorum for a value
    Value(ValueId),
}

/// Returns whether or note `value > (2/3)*total`.
fn is_quorum(value: Weight, total: Weight) -> bool {
    3 * value > 2 * total
}

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;

    #[test]
    fn values_weights() {
        let mut vw = ValuesWeights::new();

        assert_eq!(vw.get(&None), 0);
        assert_eq!(vw.get(&Some(1)), 0);

        assert_eq!(vw.add(None, 1), 1);
        assert_eq!(vw.get(&None), 1);
        assert_eq!(vw.get(&Some(1)), 0);

        assert_eq!(vw.add(Some(1), 1), 1);
        assert_eq!(vw.get(&None), 1);
        assert_eq!(vw.get(&Some(1)), 1);

        assert_eq!(vw.add(None, 1), 2);
        assert_eq!(vw.get(&None), 2);
        assert_eq!(vw.get(&Some(1)), 1);

        assert_eq!(vw.add(Some(1), 1), 2);
        assert_eq!(vw.get(&None), 2);
        assert_eq!(vw.get(&Some(1)), 2);

        assert_eq!(vw.add(Some(2), 1), 1);
        assert_eq!(vw.get(&None), 2);
        assert_eq!(vw.get(&Some(1)), 2);
        assert_eq!(vw.get(&Some(2)), 1);

        // FIXME: Test for and deal with overflows
    }

    #[test]
    fn vote_count_nil() {
        let mut vc = VoteCount::new(4);

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
        let mut vc = VoteCount::new(4);

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

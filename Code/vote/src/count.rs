use alloc::collections::BTreeMap;

// TODO: Introduce newtype
// QUESTION: Over what type? i64?
pub type Weight = u64;

/// A value and the weight of votes for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuesWeights<ValueId> {
    value_weights: BTreeMap<Option<ValueId>, Weight>,
}

impl<ValueId> ValuesWeights<ValueId> {
    pub fn new() -> ValuesWeights<ValueId> {
        ValuesWeights {
            value_weights: BTreeMap::new(),
        }
    }

    /// Add weight to the value and return the new weight.
    pub fn add(&mut self, value: Option<ValueId>, weight: Weight) -> Weight
    where
        ValueId: Ord,
    {
        let entry = self.value_weights.entry(value).or_insert(0);
        *entry += weight; // FIXME: Deal with overflows
        *entry
    }

    /// Return the weight of the value, or 0 if it is not present.
    pub fn get(&self, value: &Option<ValueId>) -> Weight
    where
        ValueId: Ord,
    {
        self.value_weights.get(value).cloned().unwrap_or(0)
    }

    /// Return the sum of the weights of all values.
    pub fn sum(&self) -> Weight {
        self.value_weights.values().sum() // FIXME: Deal with overflows
    }
}

impl<ValueId> Default for ValuesWeights<ValueId> {
    fn default() -> Self {
        Self::new()
    }
}

/// VoteCount tallys votes of the same type.
/// Votes are for nil or for some value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteCount<Value> {
    /// Weight of votes for the values, including nil
    pub values_weights: ValuesWeights<Value>,

    /// Total weight
    pub total: Weight,
}

impl<Value> VoteCount<Value> {
    pub fn new(total: Weight) -> Self {
        VoteCount {
            total,
            values_weights: ValuesWeights::new(),
        }
    }

    /// Add vote for a vlaue to internal counters and return the highest threshold.
    pub fn add_vote(&mut self, value: Option<Value>, weight: Weight) -> Threshold<Value>
    where
        Value: Clone + Ord,
    {
        let new_weight = self.values_weights.add(value.clone(), weight);

        match value {
            Some(value) if is_quorum(new_weight, self.total) => Threshold::Value(value),

            None if is_quorum(new_weight, self.total) => Threshold::Nil,

            _ => {
                let sum_weight = self.values_weights.sum();

                if is_quorum(sum_weight, self.total) {
                    Threshold::Any
                } else {
                    Threshold::Init
                }
            }
        }
    }

    /// Return whether or not the threshold is met, ie. if we have a quorum for that threshold.
    pub fn is_threshold_met(&self, threshold: Threshold<Value>) -> bool
    where
        Value: Clone + Ord,
    {
        match threshold {
            Threshold::Value(value) => {
                let weight = self.values_weights.get(&Some(value));
                is_quorum(weight, self.total)
            }

            Threshold::Nil => {
                let weight = self.values_weights.get(&None);
                is_quorum(weight, self.total)
            }

            Threshold::Any => {
                let sum_weight = self.values_weights.sum();
                is_quorum(sum_weight, self.total)
            }

            Threshold::Init => false,
        }
    }
}

//-------------------------------------------------------------------------
// Round votes
//-------------------------------------------------------------------------

// Thresh represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Threshold<ValueId> {
    /// No quorum
    Init, // no quorum
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
    #[allow(clippy::bool_assert_comparison)]
    fn vote_count_nil() {
        let mut vc = VoteCount::new(4);

        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(None, 1), Threshold::Init);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(None, 1), Threshold::Init);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(None, 1), Threshold::Nil);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(Some(1), 1), Threshold::Any);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)]
    fn vote_count_value() {
        let mut vc = VoteCount::new(4);

        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(Some(1), 1), Threshold::Init);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(Some(1), 1), Threshold::Init);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), false);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(Some(1), 1), Threshold::Value(1));
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);

        assert_eq!(vc.add_vote(Some(2), 1), Threshold::Any);
        assert_eq!(vc.is_threshold_met(Threshold::Init), false);
        assert_eq!(vc.is_threshold_met(Threshold::Any), true);
        assert_eq!(vc.is_threshold_met(Threshold::Nil), false);
        assert_eq!(vc.is_threshold_met(Threshold::Value(1)), true);
        assert_eq!(vc.is_threshold_met(Threshold::Value(2)), false);
    }
}

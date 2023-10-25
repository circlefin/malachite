use alloc::collections::BTreeMap;

use malachite_common::{Consensus, ValueId, Vote};

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
        *entry += weight;
        *entry
    }

    /// Return the sum of the weights of all values.
    pub fn sum(&self) -> Weight {
        self.value_weights.values().sum()
    }

    /// Return the weight of the value, or 0 if it is not present.
    fn get(&self, value: &Option<ValueId>) -> Weight
    where
        ValueId: Ord,
    {
        self.value_weights.get(value).cloned().unwrap_or(0)
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
pub struct VoteCount<C>
where
    C: Consensus,
{
    /// Weight of votes for the values, including nil
    pub values_weights: ValuesWeights<ValueId<C>>,

    /// Total weight
    pub total: Weight,
}

impl<C> VoteCount<C>
where
    C: Consensus,
{
    pub fn new(total: Weight) -> Self {
        VoteCount {
            total,
            values_weights: ValuesWeights::new(),
        }
    }

    /// Add vote to internal counters and return the highest threshold.
    pub fn add_vote(&mut self, vote: C::Vote, weight: Weight) -> Threshold<ValueId<C>> {
        let new_weight = self.values_weights.add(vote.value().cloned(), weight);

        match vote.value() {
            Some(value) if is_quorum(new_weight, self.total) => Threshold::Value(value.clone()),

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
    pub fn is_threshold_met(&self, threshold: Threshold<ValueId<C>>) -> bool {
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
pub fn is_quorum(value: Weight, total: Weight) -> bool {
    3 * value > 2 * total
}

#[cfg(test)]
mod tests {}

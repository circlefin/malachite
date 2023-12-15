//! A value and the weight of votes for it.

use alloc::collections::BTreeMap;

use crate::Weight;

/// A value and the weight of votes for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuesWeights<Value> {
    value_weights: BTreeMap<Value, Weight>,
}

impl<Value> ValuesWeights<Value> {
    /// Create a new `ValuesWeights` instance.
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
}

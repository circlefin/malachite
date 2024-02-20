//! For tracking the weight (ie. voting power) of each validator.

use alloc::collections::BTreeMap;

use crate::Weight;

/// Keeps track of the weight (ie. voting power) of each validator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundWeights<Address> {
    map: BTreeMap<Address, Weight>,
}

impl<Address> RoundWeights<Address> {
    /// Create a new `RoundWeights` instance.
    pub fn new() -> Self {
        RoundWeights {
            map: BTreeMap::new(),
        }
    }

    /// Return the inner map.
    pub fn get_inner(&self) -> &BTreeMap<Address, Weight> {
        &self.map
    }

    /// Set the weight of the given address, if it is not already set.
    pub fn set_once(&mut self, address: Address, weight: Weight)
    where
        Address: Ord,
    {
        self.map.entry(address).or_insert(weight);
    }

    /// Get the weight of the given address.
    pub fn get(&self, address: &Address) -> Weight
    where
        Address: Ord,
    {
        *self.map.get(address).unwrap_or(&0)
    }

    /// Return the sum of the weights of all the addresses.
    pub fn sum(&self) -> Weight {
        self.map.values().sum()
    }
}

impl<Address> Default for RoundWeights<Address> {
    fn default() -> Self {
        Self::new()
    }
}

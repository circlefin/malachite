use alloc::collections::BTreeMap;

use crate::Weight;

#[derive(Clone, Debug)]
pub struct RoundWeights<Address> {
    map: BTreeMap<Address, Weight>,
}

impl<Address> RoundWeights<Address> {
    pub fn new() -> Self {
        RoundWeights {
            map: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, address: Address, weight: Weight)
    where
        Address: Ord,
    {
        self.map.entry(address).or_insert(weight);
    }

    pub fn get(&self, address: &Address) -> Weight
    where
        Address: Ord,
    {
        *self.map.get(address).unwrap_or(&0)
    }

    pub fn total(&self) -> Weight {
        self.map.values().sum()
    }
}

impl<Address> Default for RoundWeights<Address> {
    fn default() -> Self {
        Self::new()
    }
}

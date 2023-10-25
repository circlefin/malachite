use std::sync::atomic::{AtomicUsize, Ordering};

use malachite_common::VotingPower;

use crate::TestConsensus;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    pub const fn new(value: Vec<u8>) -> Self {
        Self(value)
    }

    pub fn hash(&self) -> u64 {
        self.0.iter().fold(0, |acc, x| acc ^ *x as u64)
    }
}

impl malachite_common::PublicKey for PublicKey {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address(u64);

impl Address {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

impl malachite_common::Address for Address {}

/// A validator is a public key and voting power
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub address: Address,
    pub public_key: PublicKey,
    pub voting_power: VotingPower,
}

impl Validator {
    pub fn new(public_key: PublicKey, voting_power: VotingPower) -> Self {
        Self {
            address: Address(public_key.hash()),
            public_key,
            voting_power,
        }
    }

    pub fn hash(&self) -> u64 {
        self.public_key.hash() // TODO
    }
}

impl malachite_common::Validator<TestConsensus> for Validator {
    fn address(&self) -> &Address {
        &self.address
    }

    fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    fn voting_power(&self) -> VotingPower {
        self.voting_power
    }
}

/// A validator set contains a list of validators sorted by address.
pub struct ValidatorSet {
    validators: Vec<Validator>,
    proposer: AtomicUsize,
}

impl ValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = Validator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        ValidatorSet::sort_validators(&mut validators);

        assert!(!validators.is_empty());

        Self {
            validators,
            proposer: AtomicUsize::new(0),
        }
    }

    /// The total voting power of the validator set
    pub fn total_voting_power(&self) -> VotingPower {
        // TODO: Cache this?
        self.validators.iter().map(|v| v.voting_power).sum()
    }

    /// Add a validator to the set
    pub fn add(&mut self, validator: Validator) {
        self.validators.push(validator);

        ValidatorSet::sort_validators(&mut self.validators);
    }

    /// Update the voting power of the given validator
    pub fn update(&mut self, val: Validator) {
        if let Some(v) = self
            .validators
            .iter_mut()
            .find(|v| v.address == val.address)
        {
            v.voting_power = val.voting_power;
        }

        dbg!(self.total_voting_power());
        Self::sort_validators(&mut self.validators);
        dbg!(self.total_voting_power());
    }

    /// Remove a validator from the set
    pub fn remove(&mut self, address: &Address) {
        self.validators.retain(|v| &v.address != address);

        Self::sort_validators(&mut self.validators); // TODO: Not needed
    }

    /// Get a validator by its address
    pub fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.validators.iter().find(|v| &v.address == address)
    }

    pub fn get_by_public_key(&self, public_key: &PublicKey) -> Option<&Validator> {
        self.validators.iter().find(|v| &v.public_key == public_key)
    }

    /// In place sort and deduplication of a list of validators
    fn sort_validators(vals: &mut Vec<Validator>) {
        use core::cmp::Reverse;

        // Sort the validators according to the current Tendermint requirements
        // (v. 0.34 -> first by validator power, descending, then by address, ascending)
        vals.sort_unstable_by_key(|v| (Reverse(v.voting_power), v.address));

        vals.dedup();
    }

    pub fn get_proposer(&self) -> Validator {
        // TODO: Proper implementation
        assert!(!self.validators.is_empty());
        let proposer = self.validators[self.proposer.load(Ordering::Relaxed)].clone();
        self.proposer.fetch_add(1, Ordering::Relaxed);
        proposer
    }
}

impl malachite_common::ValidatorSet<TestConsensus> for ValidatorSet {
    fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power()
    }

    fn get_by_public_key(&self, public_key: &PublicKey) -> Option<&Validator> {
        self.get_by_public_key(public_key)
    }

    fn get_proposer(&self) -> Validator {
        self.get_proposer()
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.get_by_address(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_update_remove() {
        let v1 = Validator::new(PublicKey(vec![1]), 1);
        let v2 = Validator::new(PublicKey(vec![2]), 2);
        let v3 = Validator::new(PublicKey(vec![3]), 3);

        let mut vs = ValidatorSet::new(vec![v1, v2, v3]);
        assert_eq!(vs.total_voting_power(), 6);

        let v4 = Validator::new(PublicKey(vec![4]), 4);
        vs.add(v4);
        assert_eq!(vs.total_voting_power(), 10);

        let mut v5 = Validator::new(PublicKey(vec![5]), 5);
        vs.update(v5.clone()); // no effect
        assert_eq!(vs.total_voting_power(), 10);

        vs.add(v5.clone());
        assert_eq!(vs.total_voting_power(), 15);

        v5.voting_power = 100;
        vs.update(v5.clone());
        assert_eq!(vs.total_voting_power(), 110);

        vs.remove(&v5.address);
        assert_eq!(vs.total_voting_power(), 10);

        let v6 = Validator::new(PublicKey(vec![6]), 6);
        vs.remove(&v6.address); // no effect
        assert_eq!(vs.total_voting_power(), 10);
    }
}

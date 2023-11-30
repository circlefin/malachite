use core::fmt;

use malachite_common::VotingPower;

use crate::{signing::PublicKey, TestContext};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address([u8; Self::LENGTH]);

impl Address {
    const LENGTH: usize = 20;

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn new(value: [u8; Self::LENGTH]) -> Self {
        Self(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_public_key(public_key: &PublicKey) -> Self {
        let hash = public_key.hash();
        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&hash[..Self::LENGTH]);
        Self(address)
    }
}

impl fmt::Display for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(public_key: PublicKey, voting_power: VotingPower) -> Self {
        Self {
            address: Address::from_public_key(&public_key),
            public_key,
            voting_power,
        }
    }
}

impl malachite_common::Validator<TestContext> for Validator {
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,
}

impl ValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = Validator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        ValidatorSet::sort_validators(&mut validators);

        assert!(!validators.is_empty());

        Self { validators }
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

        Self::sort_validators(&mut self.validators);
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
        // Sort the validators according to the current Tendermint requirements
        //
        // use core::cmp::Reverse;
        //
        // (v. 0.34 -> first by validator power, descending, then by address, ascending)
        // vals.sort_unstable_by(|v1, v2| {
        //     let a = (Reverse(v1.voting_power), &v1.address);
        //     let b = (Reverse(v2.voting_power), &v2.address);
        //     a.cmp(&b)
        // });

        vals.dedup();
    }
}

impl malachite_common::ValidatorSet<TestContext> for ValidatorSet {
    fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power()
    }

    fn get_by_public_key(&self, public_key: &PublicKey) -> Option<&Validator> {
        self.get_by_public_key(public_key)
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.get_by_address(address)
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;

    use crate::PrivateKey;

    #[test]
    fn add_update_remove() {
        let mut rng = StdRng::seed_from_u64(0x42);

        let sk1 = PrivateKey::generate(&mut rng);
        let sk2 = PrivateKey::generate(&mut rng);
        let sk3 = PrivateKey::generate(&mut rng);
        let sk4 = PrivateKey::generate(&mut rng);
        let sk5 = PrivateKey::generate(&mut rng);
        let sk6 = PrivateKey::generate(&mut rng);

        let v1 = Validator::new(sk1.public_key(), 1);
        let v2 = Validator::new(sk2.public_key(), 2);
        let v3 = Validator::new(sk3.public_key(), 3);

        let mut vs = ValidatorSet::new(vec![v1, v2, v3]);
        assert_eq!(vs.total_voting_power(), 6);

        let v4 = Validator::new(sk4.public_key(), 4);
        vs.add(v4);
        assert_eq!(vs.total_voting_power(), 10);

        let mut v5 = Validator::new(sk5.public_key(), 5);
        vs.update(v5.clone()); // no effect
        assert_eq!(vs.total_voting_power(), 10);

        vs.add(v5.clone());
        assert_eq!(vs.total_voting_power(), 15);

        v5.voting_power = 100;
        vs.update(v5.clone());
        assert_eq!(vs.total_voting_power(), 110);

        vs.remove(&v5.address);
        assert_eq!(vs.total_voting_power(), 10);

        let v6 = Validator::new(sk6.public_key(), 6);
        vs.remove(&v6.address); // no effect
        assert_eq!(vs.total_voting_power(), 10);
    }
}

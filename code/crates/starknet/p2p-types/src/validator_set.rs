use malachite_core_types::VotingPower;
use serde::{Deserialize, Serialize};

use crate::{Address, PublicKey, Validator};

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

        Self::sort_validators(&mut self.validators);
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
        use core::cmp::Reverse;

        // first by validator power descending, then by address ascending
        vals.sort_unstable_by(|v1, v2| {
            let a = (Reverse(v1.voting_power), &v1.address);
            let b = (Reverse(v2.voting_power), &v2.address);
            a.cmp(&b)
        });

        vals.dedup();
    }

    pub fn get_keys(&self) -> Vec<PublicKey> {
        self.validators.iter().map(|v| v.public_key).collect()
    }
}

impl Serialize for ValidatorSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ValidatorSet<'a> {
            validators: &'a [Validator],
        }

        let vs = ValidatorSet {
            validators: &self.validators,
        };

        vs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ValidatorSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct ValidatorSet {
            validators: Vec<Validator>,
        }

        ValidatorSet::deserialize(deserializer).map(|vs| Self::new(vs.validators))
    }
}

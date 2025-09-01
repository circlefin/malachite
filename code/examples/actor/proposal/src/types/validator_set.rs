use core::slice;
use std::sync::Arc;

use malachitebft_core_types::VotingPower;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::{address::Address, context::MockContext, signing::PublicKey};

/// A validator is a public key and voting power
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl PartialOrd for Validator {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Validator {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.address.cmp(&other.address)
    }
}

impl malachitebft_core_types::Validator<MockContext> for Validator {
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
    pub validators: Arc<Vec<Validator>>,
}

impl Serialize for ValidatorSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validators.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ValidatorSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let validators = Vec::<Validator>::deserialize(deserializer)?;
        Ok(Self {
            validators: Arc::new(validators),
        })
    }
}

impl ValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = Validator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        ValidatorSet::sort_validators(&mut validators);

        assert!(!validators.is_empty());

        Self {
            validators: Arc::new(validators),
        }
    }

    /// Get the number of validators in the set
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Iterate over the validators in the set
    pub fn iter(&self) -> slice::Iter<Validator> {
        self.validators.iter()
    }

    /// The total voting power of the validator set
    pub fn total_voting_power(&self) -> VotingPower {
        self.validators.iter().map(|v| v.voting_power).sum()
    }

    /// Get a validator by its index
    pub fn get_by_index(&self, index: usize) -> Option<&Validator> {
        self.validators.get(index)
    }

    /// Get a validator by its address
    pub fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.validators.iter().find(|v| &v.address == address)
    }

    fn sort_validators(vals: &mut Vec<Validator>) {
        // Sort the validators according to the current Tendermint requirements
        //
        // use core::cmp::Reverse;
        //
        // (first by validator power, descending, then by address, ascending)
        // vals.sort_unstable_by(|v1, v2| {
        //     let a = (Reverse(v1.voting_power), &v1.address);
        //     let b = (Reverse(v2.voting_power), &v2.address);
        //     a.cmp(&b)
        // });

        vals.dedup();
    }
}

impl malachitebft_core_types::ValidatorSet<MockContext> for ValidatorSet {
    fn count(&self) -> usize {
        self.validators.len()
    }

    fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power()
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.get_by_address(address)
    }

    fn get_by_index(&self, index: usize) -> Option<&Validator> {
        self.validators.get(index)
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;

    use crate::types::signing::PrivateKey;

    #[test]
    fn new_validator_set_vp() {
        let mut rng = StdRng::seed_from_u64(0x42);

        let sk1 = PrivateKey::generate(&mut rng);
        let sk2 = PrivateKey::generate(&mut rng);
        let sk3 = PrivateKey::generate(&mut rng);

        let v1 = Validator::new(sk1.public_key(), 1);
        let v2 = Validator::new(sk2.public_key(), 2);
        let v3 = Validator::new(sk3.public_key(), 3);

        let vs = ValidatorSet::new(vec![v1, v2, v3]);
        assert_eq!(vs.total_voting_power(), 6);
    }
}

use malachitebft_core_types::VotingPower;
use serde::{Deserialize, Serialize};

use crate::{Address, PublicKey};

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
            address: Address::from_public_key(public_key),
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

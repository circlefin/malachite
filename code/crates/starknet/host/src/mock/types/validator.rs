use malachite_common::{self as common, VotingPower};

use crate::mock::types::{Address, PublicKey, StarknetContext};

pub use malachite_test::Validator;

impl common::Validator<StarknetContext> for Validator {
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

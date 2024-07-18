use malachite_common::{self as common};

use crate::mock::types::{Address, StarknetContext, Validator};

pub use malachite_test::ValidatorSet;

impl common::ValidatorSet<StarknetContext> for ValidatorSet {
    fn count(&self) -> usize {
        self.validators.len()
    }

    fn total_voting_power(&self) -> common::VotingPower {
        self.total_voting_power()
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.get_by_address(address)
    }

    fn get_by_index(&self, index: usize) -> Option<&Validator> {
        self.validators.get(index)
    }
}

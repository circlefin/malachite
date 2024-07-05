// Not used at the moment..

use malachite_starknet_host::mock::host::MockParams;
use malachite_starknet_host::mock::types::ValidatorSet;

/// Has no mempool reference, since we'll use a mempool that is purely local.
/// Reuses the MockParams from [`malachite_starknet_host`].
///
pub struct KvHost {
    params: MockParams,
    validator_set: ValidatorSet,
}

impl KvHost {
    pub fn new(params: MockParams, validator_set: ValidatorSet) -> Self {
        Self {
            params,
            validator_set,
        }
    }

    pub fn params(&self) -> MockParams {
        self.params
    }

    pub fn validator_set(&self) -> ValidatorSet {
        self.validator_set.clone()
    }
}

// impl Host for KvHost
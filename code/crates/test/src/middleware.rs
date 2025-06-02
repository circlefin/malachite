use core::fmt;

use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{CommitCertificate, NilOrVal, Round};

use crate::{Address, Genesis, Height, Proposal, TestContext, ValidatorSet, Value, ValueId, Vote};

pub trait Middleware: fmt::Debug + Send + Sync {
    fn get_validator_set(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        _height: Height,
        genesis: &Genesis,
    ) -> Option<ValidatorSet> {
        Some(genesis.validator_set.clone())
    }

    fn new_proposal(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        Proposal::new(height, round, value, pol_round, address)
    }

    fn new_prevote(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }

    fn on_propose_value(
        &self,
        _ctx: &TestContext,
        _proposed_value: &mut LocallyProposedValue<TestContext>,
        _reproposal: bool,
    ) {
    }

    fn on_commit(
        &self,
        _ctx: &TestContext,
        _certificate: &CommitCertificate<TestContext>,
        _proposal: &ProposedValue<TestContext>,
    ) -> Result<(), eyre::Report> {
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DefaultMiddleware;

impl Middleware for DefaultMiddleware {}

fn select_validators(genesis: &Genesis, height: Height, selection_size: usize) -> ValidatorSet {
    let num_validators = genesis.validator_set.len();

    if num_validators <= selection_size {
        return genesis.validator_set.clone();
    }

    ValidatorSet::new(
        genesis
            .validator_set
            .iter()
            .cycle()
            .skip(height.as_u64() as usize % num_validators)
            .take(selection_size)
            .cloned()
            .collect::<Vec<_>>(),
    )
}

#[derive(Copy, Clone, Debug)]
pub struct RotateValidators {
    pub selection_size: usize,
}

impl Middleware for RotateValidators {
    // Selects N validators from index height % num_validators (circularly).
    // Example:
    //   - N = 3, num_validators = 5, height = 0 -> [0, 1, 2]
    //   - N = 3, num_validators = 5, height = 3 -> [3, 4, 0]
    fn get_validator_set(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        height: Height,
        genesis: &Genesis,
    ) -> Option<ValidatorSet> {
        let num_validators = genesis.validator_set.len();

        if num_validators <= self.selection_size {
            return Some(genesis.validator_set.clone());
        }

        Some(select_validators(genesis, height, self.selection_size))
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EpochValidators {
    pub epochs_limit: usize,
}

impl Middleware for EpochValidators {
    fn get_validator_set(
        &self,
        _ctx: &TestContext,
        current_height: Height,
        height: Height,
        genesis: &Genesis,
    ) -> Option<ValidatorSet> {
        if height.as_u64() > current_height.as_u64() + self.epochs_limit as u64 {
            return None;
        }

        Some(genesis.validator_set.clone())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RotateEpochValidators {
    pub selection_size: usize,
    pub epochs_limit: usize,
}

impl Middleware for RotateEpochValidators {
    fn get_validator_set(
        &self,
        _ctx: &TestContext,
        current_height: Height,
        height: Height,
        genesis: &Genesis,
    ) -> Option<ValidatorSet> {
        if height.as_u64() > current_height.as_u64() + self.epochs_limit as u64 {
            return None;
        }

        let num_validators = genesis.validator_set.len();

        if num_validators <= self.selection_size {
            return Some(genesis.validator_set.clone());
        }

        Some(select_validators(genesis, height, self.selection_size))
    }
}

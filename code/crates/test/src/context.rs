use std::sync::Arc;

use bytes::Bytes;

use malachitebft_core_types::{Context, NilOrVal, Round, ValidatorSet as _};

use crate::address::*;
use crate::height::*;
use crate::middleware;
use crate::middleware::Middleware;
use crate::proposal::*;
use crate::proposal_part::*;
use crate::signing::*;
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Clone, Debug)]
pub struct TestContext {
    middleware: Arc<dyn Middleware>,
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TestContext {
    pub fn new() -> Self {
        Self::with_middleware(Arc::new(middleware::DefaultMiddleware))
    }

    pub fn with_middleware(middleware: Arc<dyn Middleware>) -> Self {
        Self { middleware }
    }

    pub fn middleware(&self) -> &Arc<dyn Middleware> {
        &self.middleware
    }

    pub fn select_proposer<'a>(
        &self,
        validator_set: &'a ValidatorSet,
        height: Height,
        round: Round,
    ) -> &'a Validator {
        assert!(validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let proposer_index = {
            let height = height.as_u64() as usize;
            let round = round.as_i64() as usize;

            (height - 1 + round) % validator_set.count()
        };

        validator_set
            .get_by_index(proposer_index)
            .expect("proposer_index is valid")
    }
}

impl Context for TestContext {
    type Address = Address;
    type ProposalPart = ProposalPart;
    type Height = Height;
    type Proposal = Proposal;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = Value;
    type Vote = Vote;
    type Extension = Bytes;
    type SigningScheme = Ed25519;

    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator {
        self.select_proposer(validator_set, height, round)
    }

    fn new_proposal(
        &self,
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        self.middleware
            .new_proposal(self, height, round, value, pol_round, address)
    }

    fn new_prevote(
        &self,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        self.middleware
            .new_prevote(self, height, round, value_id, address)
    }

    fn new_precommit(
        &self,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        self.middleware
            .new_precommit(self, height, round, value_id, address)
    }
}

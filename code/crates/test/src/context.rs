use std::sync::Arc;

use malachitebft_core_types::{Context, NilOrVal, Round, ValidatorSet as _};

use crate::address::*;
use crate::height::*;
use crate::proposal::*;
use crate::proposal_part::*;
use crate::signing::*;
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Clone, Debug)]
pub struct TestContext {
    pub signing_provider: Arc<Ed25519Provider>,
}

impl TestContext {
    pub fn new(private_key: PrivateKey) -> Self {
        Self {
            signing_provider: Arc::new(Ed25519Provider::new(private_key)),
        }
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
    type SigningScheme = Ed25519;
    type SigningProvider = Ed25519Provider;

    fn signing_provider(&self) -> &Self::SigningProvider {
        &self.signing_provider
    }

    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator {
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

    fn new_proposal(
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        Proposal::new(height, round, value, pol_round, address)
    }

    fn new_prevote(
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }
}

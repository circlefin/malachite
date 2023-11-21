use malachite_common::Context;
use malachite_common::Round;
use malachite_common::SignedVote;

use crate::height::*;
use crate::proposal::*;
use crate::signing::{Ed25519, PrivateKey, PublicKey};
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Clone, Debug)]
pub struct TestContext {
    private_key: PrivateKey,
}

impl TestContext {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }
}

impl Context for TestContext {
    type Address = Address;
    type Height = Height;
    type Proposal = Proposal;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = Value;
    type Vote = Vote;
    type SigningScheme = Ed25519;

    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&vote.to_bytes());
        SignedVote::new(vote, signature)
    }

    fn verify_signed_vote(&self, signed_vote: &SignedVote<Self>, public_key: &PublicKey) -> bool {
        use signature::Verifier;
        public_key
            .verify(&signed_vote.vote.to_bytes(), &signed_vote.signature)
            .is_ok()
    }

    fn new_proposal(height: Height, round: Round, value: Value, pol_round: Round) -> Proposal {
        Proposal::new(height, round, value, pol_round)
    }

    fn new_prevote(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }
}

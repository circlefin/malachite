use malachite_common::Context;
use malachite_common::Round;
use malachite_common::SignedVote;

use crate::height::*;
use crate::proposal::*;
use crate::signing::{Ed25519, PrivateKey, PublicKey, Signature};
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestConsensus;

impl Context for TestConsensus {
    type Address = Address;
    type Height = Height;
    type Proposal = Proposal;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = Value;
    type Vote = Vote;
    type SigningScheme = Ed25519;

    const DUMMY_VALUE: Self::Value = Value::new(9999);

    fn sign_vote(vote: &Self::Vote, private_key: &PrivateKey) -> Signature {
        use signature::Signer;
        private_key.sign(&vote.to_bytes())
    }

    fn verify_signed_vote(signed_vote: &SignedVote<Self>, public_key: &PublicKey) -> bool {
        use signature::Verifier;
        public_key
            .verify(&signed_vote.vote.to_bytes(), &signed_vote.signature)
            .is_ok()
    }

    fn new_proposal(height: Height, round: Round, value: Value, pol_round: Round) -> Proposal {
        Proposal::new(height, round, value, pol_round)
    }

    fn new_prevote(round: Round, value_id: Option<ValueId>, address: Address) -> Vote {
        Vote::new_prevote(round, value_id, address)
    }

    fn new_precommit(round: Round, value_id: Option<ValueId>, address: Address) -> Vote {
        Vote::new_precommit(round, value_id, address)
    }
}

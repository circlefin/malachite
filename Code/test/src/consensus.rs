use malachite_common::Consensus;
use malachite_common::Round;
use malachite_common::SignedVote;

use crate::height::*;
use crate::proposal::*;
use crate::public_key::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestConsensus;

impl Consensus for TestConsensus {
    type Address = Address;
    type Height = Height;
    type Proposal = Proposal;
    type PublicKey = Ed25519PublicKey;
    type PrivateKey = Ed25519PrivateKey;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = Value;
    type Vote = Vote;

    const DUMMY_VALUE: Self::Value = Value::new(9999);

    fn sign_vote(vote: &Self::Vote, private_key: &Self::PrivateKey) -> Ed25519Signature {
        use signature::Signer;
        private_key.sign(&vote.to_bytes())
    }

    fn verify_signed_vote(signed_vote: &SignedVote<Self>, public_key: &Ed25519PublicKey) -> bool {
        use signature::Verifier;
        public_key
            .verify(&signed_vote.vote.to_bytes(), &signed_vote.signature)
            .is_ok()
    }

    fn new_proposal(height: Height, round: Round, value: Value, pol_round: Round) -> Proposal {
        Proposal::new(height, round, value, pol_round)
    }

    fn new_prevote(round: Round, value_id: Option<ValueId>) -> Vote {
        Vote::new_prevote(round, value_id)
    }

    fn new_precommit(round: Round, value_id: Option<ValueId>) -> Vote {
        Vote::new_precommit(round, value_id)
    }
}

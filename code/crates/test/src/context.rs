use std::sync::Arc;

use malachite_common::{Context, NilOrVal, Round, SignedBlockPart, SignedProposal, SignedVote};

use crate::address::*;
use crate::height::*;
use crate::proposal::*;
use crate::signing::*;
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;
use crate::BlockPart;

#[derive(Clone, Debug)]
pub struct TestContext {
    private_key: Arc<PrivateKey>,
}

impl TestContext {
    pub fn new(private_key: PrivateKey) -> Self {
        Self {
            private_key: Arc::new(private_key),
        }
    }
}

impl Context for TestContext {
    type Address = Address;
    type BlockPart = BlockPart;
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

    fn sign_proposal(&self, proposal: Self::Proposal) -> SignedProposal<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&proposal.to_bytes());
        SignedProposal::new(proposal, signature)
    }

    fn verify_signed_proposal(
        &self,
        signed_proposal: &SignedProposal<Self>,
        public_key: &PublicKey,
    ) -> bool {
        use signature::Verifier;
        public_key
            .verify(
                &signed_proposal.proposal.to_bytes(),
                &signed_proposal.signature,
            )
            .is_ok()
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

    fn sign_block_part(&self, block_part: Self::BlockPart) -> SignedBlockPart<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&block_part.to_bytes());
        SignedBlockPart::new(block_part, signature)
    }

    fn verify_signed_block_part(
        &self,
        signed_block_part: &SignedBlockPart<Self>,
        public_key: &malachite_common::PublicKey<Self>,
    ) -> bool {
        use signature::Verifier;
        public_key
            .verify(
                &signed_block_part.block_part.to_bytes(),
                &signed_block_part.signature,
            )
            .is_ok()
    }
}

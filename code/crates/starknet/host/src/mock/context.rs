use std::sync::Arc;

use malachite_common::{Context, NilOrVal, Round, SignedProposal, SignedProposalPart, SignedVote};
use malachite_starknet_p2p_types::{PublicKey, SigningScheme};
use malachite_test::PrivateKey;

use crate::types::{
    Address, BlockHash, Height, Proposal, ProposalPart, Validator, ValidatorSet, Vote,
};

#[derive(Clone, Debug)]
pub struct MockContext {
    private_key: Arc<PrivateKey>,
}

impl MockContext {
    pub fn new(private_key: PrivateKey) -> Self {
        Self {
            private_key: Arc::new(private_key),
        }
    }
}

impl Context for MockContext {
    type Address = Address;
    type ProposalPart = ProposalPart;
    type Height = Height;
    type Proposal = Proposal;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = BlockHash;
    type Vote = Vote;
    type SigningScheme = SigningScheme;

    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&vote.to_sign_bytes());
        SignedVote::new(vote, signature)
    }

    fn verify_signed_vote(&self, signed_vote: &SignedVote<Self>, public_key: &PublicKey) -> bool {
        use signature::Verifier;
        public_key
            .verify(&signed_vote.vote.to_sign_bytes(), &signed_vote.signature)
            .is_ok()
    }

    fn sign_proposal(&self, proposal: Self::Proposal) -> SignedProposal<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&proposal.to_sign_bytes());
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
                &signed_proposal.proposal.to_sign_bytes(),
                &signed_proposal.signature,
            )
            .is_ok()
    }

    fn new_proposal(
        height: Height,
        round: Round,
        block_hash: BlockHash,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        Proposal::new(height, round, block_hash, pol_round, address)
    }

    fn new_prevote(
        height: Height,
        round: Round,
        value_id: NilOrVal<BlockHash>,
        address: Address,
    ) -> Vote {
        let fork_id = 1; // FIXME: p2p-types
        Vote::new_prevote(height, round, fork_id, value_id, address)
    }

    fn new_precommit(
        height: Height,
        round: Round,
        value_id: NilOrVal<BlockHash>,
        address: Address,
    ) -> Vote {
        let fork_id = 1; // FIXME: p2p-types
        Vote::new_precommit(height, round, fork_id, value_id, address)
    }

    fn sign_proposal_part(&self, proposal_part: Self::ProposalPart) -> SignedProposalPart<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&proposal_part.to_sign_bytes());
        SignedProposalPart::new(proposal_part, signature)
    }

    fn verify_signed_proposal_part(
        &self,
        signed_proposal_part: &SignedProposalPart<Self>,
        public_key: &malachite_common::PublicKey<Self>,
    ) -> bool {
        use signature::Verifier;
        public_key
            .verify(
                &signed_proposal_part.proposal_part.to_sign_bytes(),
                &signed_proposal_part.signature,
            )
            .is_ok()
    }
}

use std::sync::Arc;

use malachite_common::{Context, NilOrVal, Round, SignedProposal, SignedProposalPart, SignedVote};
use malachite_starknet_p2p_types::{PrivateKey, PublicKey, Signature, SigningScheme};
use starknet_core::utils::starknet_keccak;

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
        let hash = starknet_keccak(&vote.to_sign_bytes());
        let signature = self.private_key.sign(&hash);
        SignedVote::new(vote, signature)
    }

    fn verify_signed_vote(
        &self,
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        let hash = starknet_keccak(&vote.to_sign_bytes());
        public_key.verify(&hash, signature)
    }

    fn sign_proposal(&self, proposal: Self::Proposal) -> SignedProposal<Self> {
        let hash = starknet_keccak(&proposal.to_sign_bytes());
        let signature = self.private_key.sign(&hash);
        SignedProposal::new(proposal, signature)
    }

    fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        let hash = starknet_keccak(&proposal.to_sign_bytes());
        public_key.verify(&hash, signature)
    }

    fn sign_proposal_part(&self, proposal_part: Self::ProposalPart) -> SignedProposalPart<Self> {
        let hash = starknet_keccak(&proposal_part.to_sign_bytes());
        let signature = self.private_key.sign(&hash);
        SignedProposalPart::new(proposal_part, signature)
    }

    fn verify_signed_proposal_part(
        &self,
        proposal_part: &ProposalPart,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        let hash = starknet_keccak(&proposal_part.to_sign_bytes());
        public_key.verify(&hash, signature)
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
}

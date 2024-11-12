use std::sync::Arc;

use malachite_common::{
    Context, NilOrVal, Round, SignedProposal, SignedProposalPart, SignedVote, ValidatorSet as _,
};
use starknet_core::utils::starknet_keccak;

use crate::{
    Address, BlockHash, Height, PrivateKey, Proposal, ProposalPart, PublicKey, Signature,
    SigningScheme, Validator, ValidatorSet, Vote,
};

mod impls;

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
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        height: Height,
        round: Round,
        value_id: NilOrVal<BlockHash>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }
}

use starknet_core::utils::starknet_keccak;

use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, NilOrVal, SignedProposal,
    SignedProposalPart, SignedVote, SigningProvider, VotingPower,
};

use crate::{
    MockContext, PrivateKey, Proposal, ProposalPart, PublicKey, Signature, Validator, Vote,
};

#[derive(Debug)]
pub struct EcdsaProvider {
    private_key: PrivateKey,
}

impl EcdsaProvider {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }
}

impl SigningProvider<MockContext> for EcdsaProvider {
    fn sign_vote(&self, vote: Vote) -> SignedVote<MockContext> {
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

    fn sign_proposal(&self, proposal: Proposal) -> SignedProposal<MockContext> {
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

    fn sign_proposal_part(&self, proposal_part: ProposalPart) -> SignedProposalPart<MockContext> {
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

    fn verify_commit_signature(
        &self,
        certificate: &CommitCertificate<MockContext>,
        commit_sig: &CommitSignature<MockContext>,
        validator: &Validator,
    ) -> Result<VotingPower, CertificateError<MockContext>> {
        use malachitebft_core_types::Validator;

        // Reconstruct the vote that was signed
        let vote = Vote::new_precommit(
            certificate.height,
            certificate.round,
            NilOrVal::Val(certificate.value_id),
            *validator.address(),
        );

        // Verify signature
        if !self.verify_signed_vote(&vote, &commit_sig.signature, validator.public_key()) {
            return Err(CertificateError::InvalidSignature(commit_sig.clone()));
        }

        Ok(validator.voting_power())
    }
}

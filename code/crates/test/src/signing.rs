use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, NilOrVal, SignedProposal,
    SignedProposalPart, SignedVote, SigningProvider, VotingPower,
};
pub use malachitebft_signing_ed25519::*;

use crate::{Proposal, ProposalPart, TestContext, Validator, Vote};

pub trait Hashable {
    type Output;
    fn hash(&self) -> Self::Output;
}

impl Hashable for PublicKey {
    type Output = [u8; 32];

    fn hash(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(self.as_bytes());
        hasher.finalize().into()
    }
}

#[derive(Debug)]
pub struct Ed25519Provider {
    private_key: PrivateKey,
}

impl Ed25519Provider {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.private_key.sign(data)
    }

    pub fn verify(&self, data: &[u8], signature: &Signature, public_key: &PublicKey) -> bool {
        public_key.verify(data, signature).is_ok()
    }
}

impl SigningProvider<TestContext> for Ed25519Provider {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_vote(&self, vote: Vote) -> SignedVote<TestContext> {
        let signature = self.sign(&vote.to_bytes());
        SignedVote::new(vote, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_vote(
        &self,
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key.verify(&vote.to_bytes(), signature).is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_proposal(&self, proposal: Proposal) -> SignedProposal<TestContext> {
        let signature = self.private_key.sign(&proposal.to_bytes());
        SignedProposal::new(proposal, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key.verify(&proposal.to_bytes(), signature).is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_proposal_part(&self, proposal_part: ProposalPart) -> SignedProposalPart<TestContext> {
        let signature = self.private_key.sign(&proposal_part.to_sign_bytes());
        SignedProposalPart::new(proposal_part, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_proposal_part(
        &self,
        proposal_part: &ProposalPart,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key
            .verify(&proposal_part.to_sign_bytes(), signature)
            .is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_commit_signature(
        &self,
        certificate: &CommitCertificate<TestContext>,
        commit_sig: &CommitSignature<TestContext>,
        validator: &Validator,
    ) -> Result<VotingPower, CertificateError<TestContext>> {
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

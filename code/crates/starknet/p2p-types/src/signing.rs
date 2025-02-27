use bytes::Bytes;
use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, NilOrVal, SignedExtension,
    SignedProposal, SignedProposalPart, SignedVote, SigningProvider, VotingPower,
};

pub use malachitebft_signing_ed25519::{Ed25519, PrivateKey, PublicKey, Signature};

use crate::{MockContext, Proposal, ProposalPart, Validator, Vote};

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

impl SigningProvider<MockContext> for Ed25519Provider {
    fn sign_vote(&self, vote: Vote) -> SignedVote<MockContext> {
        // Votes are not signed for now
        SignedVote::new(vote, Signature::test())
    }

    fn verify_signed_vote(
        &self,
        _vote: &Vote,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Votes are not signed for now
        true
    }

    fn sign_proposal(&self, proposal: Proposal) -> SignedProposal<MockContext> {
        // Proposals are never sent over the network
        SignedProposal::new(proposal, Signature::test())
    }

    fn verify_signed_proposal(
        &self,
        _proposal: &Proposal,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Proposals are never sent over the network
        true
    }

    fn sign_proposal_part(&self, proposal_part: ProposalPart) -> SignedProposalPart<MockContext> {
        // Proposal parts are not signed for now
        SignedProposalPart::new(proposal_part, Signature::test())
    }

    fn verify_signed_proposal_part(
        &self,
        _proposal_part: &ProposalPart,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Proposal parts are not signed for now
        true
    }

    fn sign_vote_extension(&self, extension: Bytes) -> SignedExtension<MockContext> {
        // Vote extensions are not enabled
        SignedExtension::new(extension, Signature::test())
    }

    fn verify_signed_vote_extension(
        &self,
        _extension: &Bytes,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Vote extensions are not enabled
        true
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

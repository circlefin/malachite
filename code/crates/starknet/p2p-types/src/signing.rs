use async_trait::async_trait;
use bytes::Bytes;
use malachitebft_core_types::{SignedExtension, SignedProposal, SignedProposalPart, SignedVote};

use malachitebft_signing::{Error, SigningProvider, VerificationResult};
pub use malachitebft_signing_ed25519::{Ed25519, PrivateKey, PublicKey, Signature};

use crate::{MockContext, Proposal, ProposalPart, Vote};

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

#[async_trait]
impl SigningProvider<MockContext> for Ed25519Provider {
    async fn sign_vote(&self, vote: Vote) -> Result<SignedVote<MockContext>, Error> {
        // Votes are not signed for now
        Ok(SignedVote::new(vote, Signature::test()))
    }

    async fn verify_signed_vote(
        &self,
        _vote: &Vote,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        // Votes are not signed for now
        Ok(VerificationResult::Valid)
    }

    async fn sign_proposal(
        &self,
        proposal: Proposal,
    ) -> Result<SignedProposal<MockContext>, Error> {
        // Proposals are never sent over the network
        Ok(SignedProposal::new(proposal, Signature::test()))
    }

    async fn verify_signed_proposal(
        &self,
        _proposal: &Proposal,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        // Proposals are never sent over the network
        Ok(VerificationResult::Valid)
    }

    async fn sign_proposal_part(
        &self,
        proposal_part: ProposalPart,
    ) -> Result<SignedProposalPart<MockContext>, Error> {
        // Proposal parts are not signed for now
        Ok(SignedProposalPart::new(proposal_part, Signature::test()))
    }

    async fn verify_signed_proposal_part(
        &self,
        _proposal_part: &ProposalPart,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        // Proposal parts are not signed for now
        Ok(VerificationResult::Valid)
    }

    async fn sign_vote_extension(
        &self,
        extension: Bytes,
    ) -> Result<SignedExtension<MockContext>, Error> {
        // Vote extensions are not enabled
        Ok(SignedExtension::new(extension, Signature::test()))
    }

    async fn verify_signed_vote_extension(
        &self,
        _extension: &Bytes,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        // Vote extensions are not enabled
        Ok(VerificationResult::Valid)
    }
}

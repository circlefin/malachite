use async_trait::async_trait;
use bytes::Bytes;
use malachitebft_core_types::{SignedExtension, SignedProposal, SignedVote};

use malachitebft_signing::{Error, Signer, VerificationResult, Verifier};
pub use malachitebft_signing_ed25519::{Ed25519, PrivateKey, PublicKey, Signature};

use crate::{MockContext, Proposal, Vote};

/// Stateless signature verifier for MockContext.
#[derive(Debug)]
pub struct Ed25519Verifier;

impl Ed25519Verifier {
    pub fn verify(data: &[u8], signature: &Signature, public_key: &PublicKey) -> bool {
        public_key.verify(data, signature).is_ok()
    }
}

#[async_trait]
impl Verifier<MockContext> for Ed25519Verifier {
    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ok(VerificationResult::from_bool(Self::verify(
            bytes, signature, public_key,
        )))
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

    async fn verify_signed_proposal(
        &self,
        _proposal: &Proposal,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        // Proposals are never sent over the network
        Ok(VerificationResult::Valid)
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

/// Message signer backed by an Ed25519 private key.
/// Also implements `Verifier` for convenience.
#[derive(Debug)]
pub struct Ed25519Signer {
    private_key: PrivateKey,
}

impl Ed25519Signer {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.private_key.sign(data)
    }
}

#[async_trait]
impl Verifier<MockContext> for Ed25519Signer {
    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ed25519Verifier
            .verify_signed_bytes(bytes, signature, public_key)
            .await
    }

    async fn verify_signed_vote(
        &self,
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ed25519Verifier
            .verify_signed_vote(vote, signature, public_key)
            .await
    }

    async fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ed25519Verifier
            .verify_signed_proposal(proposal, signature, public_key)
            .await
    }

    async fn verify_signed_vote_extension(
        &self,
        extension: &Bytes,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ed25519Verifier
            .verify_signed_vote_extension(extension, signature, public_key)
            .await
    }
}

#[async_trait]
impl Signer<MockContext> for Ed25519Signer {
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature, Error> {
        Ok(self.sign(bytes))
    }

    async fn sign_vote(&self, vote: Vote) -> Result<SignedVote<MockContext>, Error> {
        // Votes are not signed for now
        Ok(SignedVote::new(vote, Signature::test()))
    }

    async fn sign_proposal(
        &self,
        proposal: Proposal,
    ) -> Result<SignedProposal<MockContext>, Error> {
        // Proposals are never sent over the network
        Ok(SignedProposal::new(proposal, Signature::test()))
    }

    async fn sign_vote_extension(
        &self,
        extension: Bytes,
    ) -> Result<SignedExtension<MockContext>, Error> {
        // Vote extensions are not enabled
        Ok(SignedExtension::new(extension, Signature::test()))
    }
}

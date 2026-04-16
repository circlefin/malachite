use async_trait::async_trait;
use bytes::Bytes;

use malachitebft_core_types::{SignedExtension, SignedProposal, SignedVote};
use malachitebft_signing::{Error, Signer, VerificationResult, Verifier};

use crate::{Proposal, TestContext, Vote};

pub use malachitebft_signing_ed25519::*;

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

/// Stateless signature verifier. Does not hold any key material —
/// all verification uses the public key passed as a parameter.
#[derive(Debug)]
pub struct Ed25519Verifier;

impl Ed25519Verifier {
    pub fn verify(data: &[u8], signature: &Signature, public_key: &PublicKey) -> bool {
        public_key.verify(data, signature).is_ok()
    }
}

#[async_trait]
impl Verifier<TestContext> for Ed25519Verifier {
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
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ok(VerificationResult::from_bool(
            public_key.verify(&vote.to_sign_bytes(), signature).is_ok(),
        ))
    }

    async fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ok(VerificationResult::from_bool(
            public_key
                .verify(&proposal.to_sign_bytes(), signature)
                .is_ok(),
        ))
    }

    async fn verify_signed_vote_extension(
        &self,
        extension: &Bytes,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<VerificationResult, Error> {
        Ok(VerificationResult::from_bool(
            public_key.verify(extension.as_ref(), signature).is_ok(),
        ))
    }
}

/// Message signer backed by an Ed25519 private key.
/// Also implements `Verifier` so it can be used where both traits are needed.
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

    pub fn verify(data: &[u8], signature: &Signature, public_key: &PublicKey) -> bool {
        Ed25519Verifier::verify(data, signature, public_key)
    }
}

#[async_trait]
impl Verifier<TestContext> for Ed25519Signer {
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
impl Signer<TestContext> for Ed25519Signer {
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature, Error> {
        Ok(self.sign(bytes))
    }

    async fn sign_vote(&self, vote: Vote) -> Result<SignedVote<TestContext>, Error> {
        let signature = self.sign(&vote.to_sign_bytes());
        Ok(SignedVote::new(vote, signature))
    }

    async fn sign_proposal(
        &self,
        proposal: Proposal,
    ) -> Result<SignedProposal<TestContext>, Error> {
        let signature = self.private_key.sign(&proposal.to_sign_bytes());
        Ok(SignedProposal::new(proposal, signature))
    }

    async fn sign_vote_extension(
        &self,
        extension: Bytes,
    ) -> Result<SignedExtension<TestContext>, Error> {
        let signature = self.private_key.sign(extension.as_ref());
        Ok(malachitebft_core_types::SignedMessage::new(
            extension, signature,
        ))
    }
}

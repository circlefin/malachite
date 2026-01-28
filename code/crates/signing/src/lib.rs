#![no_std]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

use alloc::boxed::Box;

use alloc::sync::Arc;
use async_trait::async_trait;
use malachitebft_core_types::{Context, PublicKey, Signature, SignedMessage};

mod error;
pub use error::Error;

mod ext;
pub use ext::SigningProviderExt;

/// The result of a signature verification operation.
pub enum VerificationResult {
    /// The signature is valid.
    Valid,

    /// The signature is invalid.
    Invalid,
}

impl VerificationResult {
    /// Create a new `VerificationResult` from a boolean indicating validity.
    pub fn from_bool(valid: bool) -> Self {
        if valid {
            VerificationResult::Valid
        } else {
            VerificationResult::Invalid
        }
    }

    /// Convert the result to a boolean indicating validity.
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid)
    }

    /// Convert the result to a boolean indicating invalidity.
    pub fn is_invalid(&self) -> bool {
        matches!(self, VerificationResult::Invalid)
    }
}

/// A provider of signing functionality for the consensus engine.
///
/// This trait defines the core signing operations needed by the engine,
/// including signing and verifying votes, proposals, proposal parts, and commit signatures.
/// It is parameterized by a context type `Ctx` that defines the specific types used
/// for votes, proposals, and other consensus-related data structures.
///
/// Implementers of this trait are responsible for managing the private keys used for signing
/// and providing verification logic using the corresponding public keys.
#[async_trait]
pub trait SigningProvider<Ctx>
where
    Ctx: Context,
    Self: Send + Sync,
{
    /// Sign the given bytes with our private key.
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature<Ctx>, Error>;

    /// Verify the given signature over the given bytes using the given public key.
    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Sign the given vote with our private key.
    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error>;

    /// Verify the given vote's signature using the given public key.
    async fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Sign the given proposal with our private key.
    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error>;

    /// Verify the given proposal's signature using the given public key.
    async fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Sign the given vote extension with our private key.
    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error>;

    /// Verify the given vote extension's signature using the given public key.
    async fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;
}

#[async_trait]
impl<Ctx, T> SigningProvider<Ctx> for &T
where
    T: SigningProvider<Ctx>,
    Ctx: Context,
{
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature<Ctx>, Error> {
        (*self).sign_bytes(bytes).await
    }

    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (*self)
            .verify_signed_bytes(bytes, signature, public_key)
            .await
    }

    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        (*self).sign_vote(vote).await
    }

    async fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (*self)
            .verify_signed_vote(vote, signature, public_key)
            .await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        (*self).sign_proposal(proposal).await
    }

    async fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (*self)
            .verify_signed_proposal(proposal, signature, public_key)
            .await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        (*self).sign_vote_extension(extension).await
    }

    async fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (*self)
            .verify_signed_vote_extension(extension, signature, public_key)
            .await
    }
}

#[async_trait]
impl<Ctx> SigningProvider<Ctx> for Box<dyn SigningProvider<Ctx> + '_>
where
    Ctx: Context,
{
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature<Ctx>, Error> {
        (**self).sign_bytes(bytes).await
    }

    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (**self)
            .verify_signed_bytes(bytes, signature, public_key)
            .await
    }

    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        (**self).sign_vote(vote).await
    }

    async fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_vote(vote, signature, public_key)
            .await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        self.as_ref().sign_proposal(proposal).await
    }

    async fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_proposal(proposal, signature, public_key)
            .await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        self.as_ref().sign_vote_extension(extension).await
    }

    async fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_vote_extension(extension, signature, public_key)
            .await
    }
}

#[async_trait]
impl<Ctx> SigningProvider<Ctx> for Arc<dyn SigningProvider<Ctx> + '_>
where
    Ctx: Context,
{
    async fn sign_bytes(&self, bytes: &[u8]) -> Result<Signature<Ctx>, Error> {
        (**self).sign_bytes(bytes).await
    }

    async fn verify_signed_bytes(
        &self,
        bytes: &[u8],
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (**self)
            .verify_signed_bytes(bytes, signature, public_key)
            .await
    }

    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        (**self).sign_vote(vote).await
    }

    async fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_vote(vote, signature, public_key)
            .await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        self.as_ref().sign_proposal(proposal).await
    }

    async fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_proposal(proposal, signature, public_key)
            .await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        self.as_ref().sign_vote_extension(extension).await
    }

    async fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref()
            .verify_signed_vote_extension(extension, signature, public_key)
            .await
    }
}

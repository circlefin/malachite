#![no_std]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use async_trait::async_trait;
use malachitebft_core_types::{Context, PublicKey, Signature, SignedMessage, ValidatorProof};

mod error;
pub use error::Error;

mod ext;
pub use ext::VerifierExt;

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

/// A provider of signature verification functionality for the consensus engine.
///
/// This trait defines the verification operations needed by the engine.
/// It is parameterized by a context type `Ctx` that defines the specific types used
/// for votes, proposals, and other consensus-related data structures.
///
/// All nodes (validators and non-validators) need signature verification.
///
/// Every verification method names the *purpose* of what it is verifying, so that
/// each implementation can apply the correct domain separation (e.g. network-scoped
/// for consensus messages, network-agnostic for validator proofs) without having
/// to inspect raw bytes.
#[async_trait]
pub trait Verifier<Ctx>
where
    Ctx: Context,
    Self: Send + Sync,
{
    /// Verify the given vote's signature using the given public key.
    async fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Verify the given proposal's signature using the given public key.
    async fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Verify the given vote extension's signature using the given public key.
    async fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> Result<VerificationResult, Error>;

    /// Verify a validator proof's signature using the public key included in the proof.
    ///
    /// This allows immediate verification without needing to look up the public key from
    /// the validator set. PoV is intentionally network-agnostic — implementations must
    /// verify the canonical preimage produced by [`ValidatorProof::signing_bytes`] with
    /// no domain prefix.
    async fn verify_validator_proof(
        &self,
        proof: &ValidatorProof<Ctx>,
    ) -> Result<VerificationResult, Error>;
}

/// A provider of message signing functionality for the consensus engine.
///
/// This trait defines the signing operations needed by the engine.
/// It is parameterized by a context type `Ctx` that defines the specific types used
/// for votes, proposals, and other consensus-related data structures.
///
/// Implementers of this trait are responsible for managing the private keys used for signing.
///
/// Only validator nodes need message signing.
///
/// Every signing method names the *purpose* of what it is signing, so that each
/// implementation can apply the correct domain separation without having to inspect
/// raw bytes.
#[async_trait]
pub trait Signer<Ctx>
where
    Ctx: Context,
    Self: Send + Sync,
{
    /// Sign the given vote with our private key.
    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error>;

    /// Sign the given proposal with our private key.
    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error>;

    /// Sign the given vote extension with our private key.
    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error>;

    /// Sign a validator proof binding the given public key to the given peer ID.
    ///
    /// PoV is intentionally network-agnostic — implementations must sign the canonical
    /// preimage produced by [`ValidatorProof::signing_bytes`] with no domain prefix.
    async fn sign_validator_proof(
        &self,
        public_key: Vec<u8>,
        peer_id: Vec<u8>,
    ) -> Result<ValidatorProof<Ctx>, Error>;
}

// --- Blanket impls for &T ---

#[async_trait]
impl<Ctx, T> Verifier<Ctx> for &T
where
    T: Verifier<Ctx>,
    Ctx: Context,
{
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

    async fn verify_validator_proof(
        &self,
        proof: &ValidatorProof<Ctx>,
    ) -> Result<VerificationResult, Error> {
        (*self).verify_validator_proof(proof).await
    }
}

#[async_trait]
impl<Ctx, T> Signer<Ctx> for &T
where
    T: Signer<Ctx>,
    Ctx: Context,
{
    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        (*self).sign_vote(vote).await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        (*self).sign_proposal(proposal).await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        (*self).sign_vote_extension(extension).await
    }

    async fn sign_validator_proof(
        &self,
        public_key: Vec<u8>,
        peer_id: Vec<u8>,
    ) -> Result<ValidatorProof<Ctx>, Error> {
        (*self).sign_validator_proof(public_key, peer_id).await
    }
}

// --- Blanket impls for Box<dyn ...> ---

#[async_trait]
impl<Ctx> Verifier<Ctx> for Box<dyn Verifier<Ctx> + '_>
where
    Ctx: Context,
{
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

    async fn verify_validator_proof(
        &self,
        proof: &ValidatorProof<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref().verify_validator_proof(proof).await
    }
}

#[async_trait]
impl<Ctx> Signer<Ctx> for Box<dyn Signer<Ctx> + '_>
where
    Ctx: Context,
{
    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        self.as_ref().sign_vote(vote).await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        self.as_ref().sign_proposal(proposal).await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        self.as_ref().sign_vote_extension(extension).await
    }

    async fn sign_validator_proof(
        &self,
        public_key: Vec<u8>,
        peer_id: Vec<u8>,
    ) -> Result<ValidatorProof<Ctx>, Error> {
        self.as_ref()
            .sign_validator_proof(public_key, peer_id)
            .await
    }
}

// --- Blanket impls for Arc<dyn ...> ---

#[async_trait]
impl<Ctx> Verifier<Ctx> for Arc<dyn Verifier<Ctx> + '_>
where
    Ctx: Context,
{
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

    async fn verify_validator_proof(
        &self,
        proof: &ValidatorProof<Ctx>,
    ) -> Result<VerificationResult, Error> {
        self.as_ref().verify_validator_proof(proof).await
    }
}

#[async_trait]
impl<Ctx> Signer<Ctx> for Arc<dyn Signer<Ctx> + '_>
where
    Ctx: Context,
{
    async fn sign_vote(&self, vote: Ctx::Vote) -> Result<SignedMessage<Ctx, Ctx::Vote>, Error> {
        self.as_ref().sign_vote(vote).await
    }

    async fn sign_proposal(
        &self,
        proposal: Ctx::Proposal,
    ) -> Result<SignedMessage<Ctx, Ctx::Proposal>, Error> {
        self.as_ref().sign_proposal(proposal).await
    }

    async fn sign_vote_extension(
        &self,
        extension: Ctx::Extension,
    ) -> Result<SignedMessage<Ctx, Ctx::Extension>, Error> {
        self.as_ref().sign_vote_extension(extension).await
    }

    async fn sign_validator_proof(
        &self,
        public_key: Vec<u8>,
        peer_id: Vec<u8>,
    ) -> Result<ValidatorProof<Ctx>, Error> {
        self.as_ref()
            .sign_validator_proof(public_key, peer_id)
            .await
    }
}

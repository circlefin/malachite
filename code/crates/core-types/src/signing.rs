use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::{Debug, Display};

use crate::{
    CertificateError, CommitCertificate, CommitSignature, Context, PublicKey, Signature,
    SignedMessage, ThresholdParams, VotingPower,
};

/// A signing scheme that can be used to sign votes and verify such signatures.
///
/// This trait is used to abstract over the signature scheme used by the consensus engine.
///
/// An example of a signing scheme is the Ed25519 signature scheme,
/// eg. as implemented in the [`ed25519-consensus`][ed25519-consensus] crate.
///
/// [ed25519-consensus]: https://crates.io/crates/ed25519-consensus
pub trait SigningScheme
where
    Self: Clone + Debug + Eq,
{
    /// Errors that can occur when decoding a signature from a byte array.
    type DecodingError: Display;

    /// The type of signatures produced by this signing scheme.
    type Signature: Clone + Debug + Eq + Ord + Send + Sync;

    /// The type of public keys produced by this signing scheme.
    type PublicKey: Clone + Debug + Eq + Send + Sync;

    /// The type of private keys produced by this signing scheme.
    type PrivateKey: Clone + Send + Sync;

    /// Decode a signature from a byte array.
    fn decode_signature(bytes: &[u8]) -> Result<Self::Signature, Self::DecodingError>;

    /// Encode a signature to a byte array.
    fn encode_signature(signature: &Self::Signature) -> Vec<u8>;
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
pub trait SigningProvider<Ctx>
where
    Ctx: Context,
    Self: Send + Sync + 'static,
{
    /// Sign the given vote with our private key.
    fn sign_vote(&self, vote: Ctx::Vote) -> SignedMessage<Ctx, Ctx::Vote>;

    /// Verify the given vote's signature using the given public key.
    fn verify_signed_vote(
        &self,
        vote: &Ctx::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool;

    /// Sign the given proposal with our private key.
    fn sign_proposal(&self, proposal: Ctx::Proposal) -> SignedMessage<Ctx, Ctx::Proposal>;

    /// Verify the given proposal's signature using the given public key.
    fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool;

    /// Sign the proposal part with our private key.
    fn sign_proposal_part(
        &self,
        proposal_part: Ctx::ProposalPart,
    ) -> SignedMessage<Ctx, Ctx::ProposalPart>;

    /// Verify the given proposal part signature using the given public key.
    fn verify_signed_proposal_part(
        &self,
        proposal_part: &Ctx::ProposalPart,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool;

    /// Sign the given vote extension with our private key.
    fn sign_vote_extension(&self, extension: Ctx::Extension) -> SignedMessage<Ctx, Ctx::Extension>;

    /// Verify the given vote extension's signature using the given public key.
    fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool;

    /// Verify a commit signature in a certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    fn verify_commit_signature(
        &self,
        certificate: &CommitCertificate<Ctx>,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>>;
}

impl<Ctx> SigningProvider<Ctx> for Box<dyn SigningProvider<Ctx> + '_>
where
    Ctx: Context,
{
    fn sign_vote(&self, vote: Ctx::Vote) -> SignedMessage<Ctx, Ctx::Vote> {
        self.as_ref().sign_vote(vote)
    }

    fn verify_signed_vote(
        &self,
        vote: &<Ctx as Context>::Vote,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool {
        self.as_ref()
            .verify_signed_vote(vote, signature, public_key)
    }

    fn sign_proposal(&self, proposal: Ctx::Proposal) -> SignedMessage<Ctx, Ctx::Proposal> {
        self.as_ref().sign_proposal(proposal)
    }

    fn verify_signed_proposal(
        &self,
        proposal: &Ctx::Proposal,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool {
        self.as_ref()
            .verify_signed_proposal(proposal, signature, public_key)
    }

    fn sign_proposal_part(
        &self,
        proposal_part: Ctx::ProposalPart,
    ) -> SignedMessage<Ctx, Ctx::ProposalPart> {
        self.as_ref().sign_proposal_part(proposal_part)
    }

    fn verify_signed_proposal_part(
        &self,
        proposal_part: &Ctx::ProposalPart,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool {
        self.as_ref()
            .verify_signed_proposal_part(proposal_part, signature, public_key)
    }

    fn verify_commit_signature(
        &self,
        certificate: &CommitCertificate<Ctx>,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        self.as_ref()
            .verify_commit_signature(certificate, commit_sig, validator)
    }

    fn sign_vote_extension(&self, extension: Ctx::Extension) -> SignedMessage<Ctx, Ctx::Extension> {
        self.as_ref().sign_vote_extension(extension)
    }

    fn verify_signed_vote_extension(
        &self,
        extension: &Ctx::Extension,
        signature: &Signature<Ctx>,
        public_key: &PublicKey<Ctx>,
    ) -> bool {
        self.as_ref()
            .verify_signed_vote_extension(extension, signature, public_key)
    }
}

/// Extension trait providing additional certificate verification functionality for signing providers.
///
/// This trait extends the base [`SigningProvider`] functionality with methods for verifying
/// commit certificates against validator sets. It is automatically implemented for any type
/// that implements [`SigningProvider`].
pub trait SigningProviderExt<Ctx>
where
    Ctx: Context,
{
    /// Verify the given certificate against the given validator set.
    ///
    /// - For each commit signature in the certificate:
    ///   - Reconstruct the signed precommit and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    fn verify_certificate(
        &self,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>>;
}

impl<Ctx, P> SigningProviderExt<Ctx> for P
where
    Ctx: Context,
    P: SigningProvider<Ctx>,
{
    /// Verify the certificate against the given validator set.
    ///
    /// - For each commit signature in the certificate:
    ///   - Reconstruct the signed precommit and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    fn verify_certificate(
        &self,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        use crate::ValidatorSet;

        let total_voting_power = validator_set.total_voting_power();
        let mut signed_voting_power = 0;

        // For each commit signature, reconstruct the signed precommit and verify the signature
        for commit_sig in &certificate.aggregated_signature.signatures {
            // Abort if validator not in validator set
            let Some(validator) = validator_set.get_by_address(&commit_sig.address) else {
                return Err(CertificateError::UnknownValidator(commit_sig.clone()));
            };

            let voting_power = self.verify_commit_signature(certificate, commit_sig, validator)?;
            signed_voting_power += voting_power;
        }

        // Check if we have 2/3+ voting power
        if thresholds
            .quorum
            .is_met(signed_voting_power, total_voting_power)
        {
            Ok(())
        } else {
            Err(CertificateError::NotEnoughVotingPower {
                signed: signed_voting_power,
                total: total_voting_power,
                expected: thresholds.quorum.min_expected(total_voting_power),
            })
        }
    }
}

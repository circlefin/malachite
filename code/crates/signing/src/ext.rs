use alloc::boxed::Box;
use alloc::vec::Vec;

use async_trait::async_trait;
use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, Context, NilOrVal, PolkaCertificate,
    PolkaSignature, RoundCertificate, RoundCertificateType, RoundSignature, ThresholdParams,
    Validator, ValidatorSet, VoteType, VotingPower,
};

use crate::SigningProvider;

/// Extension trait providing additional certificate verification functionality for signing providers.
///
/// This trait extends the base [`SigningProvider`] functionality with methods for verifying
/// commit certificates against validator sets. It is automatically implemented for any type
/// that implements [`SigningProvider`].
#[async_trait]
pub trait SigningProviderExt<Ctx>
where
    Ctx: Context,
{
    /// Verify a commit signature in a commit certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_commit_signature(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>>;

    /// Verify a polka signature in a polka certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_polka_signature(
        &self,
        ctx: &Ctx,
        certificate: &PolkaCertificate<Ctx>,
        signature: &PolkaSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>>;

    /// Verify a round signature in a round certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_round_signature(
        &self,
        ctx: &Ctx,
        certificate: &RoundCertificate<Ctx>,
        signature: &RoundSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>>;

    /// Verify the given certificate against the given validator set.
    ///
    /// - For each commit signature in the certificate:
    ///   - Reconstruct the signed precommit and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    async fn verify_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>>;

    /// Verify the polka certificate against the given validator set.
    ///
    /// - For each signature in the certificate:
    ///   - Reconstruct the signed prevote and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    async fn verify_polka_certificate(
        &self,
        ctx: &Ctx,
        certificate: &PolkaCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>>;

    /// Verify the round certificate against the given validator set.
    ///
    /// - For each signature in the certificate:
    ///   - Reconstruct the signed vote and verify its signature.
    /// - Check that the required voting power has signed the certificate:
    ///   - If `Precommit`, ensure that 2/3+ of the voting power is represented.
    ///   - If `Skip`, ensure that 1/3+ of the voting power is represented.
    ///  
    /// Returns a [`CertificateError`] if any verification step fails.
    async fn verify_round_certificate(
        &self,
        ctx: &Ctx,
        certificate: &RoundCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>>;
}

#[async_trait]
impl<Ctx, P> SigningProviderExt<Ctx> for P
where
    Ctx: Context,
    P: SigningProvider<Ctx>,
{
    /// Verify a commit signature in a commit certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_commit_signature(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        // Reconstruct the vote that was signed
        let vote = ctx.new_precommit(
            certificate.height,
            certificate.round,
            NilOrVal::Val(certificate.value_id.clone()),
            validator.address().clone(),
        );

        // Verify signature
        if self
            .verify_signed_vote(&vote, &commit_sig.signature, validator.public_key())
            .await
            .map_err(|e| CertificateError::VerificationError(e.into_source()))?
            .is_invalid()
        {
            return Err(CertificateError::InvalidCommitSignature(commit_sig.clone()));
        }

        Ok(validator.voting_power())
    }

    /// Verify a polka signature in a polka certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_polka_signature(
        &self,
        ctx: &Ctx,
        certificate: &PolkaCertificate<Ctx>,
        signature: &PolkaSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        // Reconstruct the vote that was signed
        let vote = ctx.new_prevote(
            certificate.height,
            certificate.round,
            NilOrVal::Val(certificate.value_id.clone()),
            validator.address().clone(),
        );

        // Verify signature
        if self
            .verify_signed_vote(&vote, &signature.signature, validator.public_key())
            .await
            .map_err(|e| CertificateError::VerificationError(e.into_source()))?
            .is_invalid()
        {
            return Err(CertificateError::InvalidPolkaSignature(signature.clone()));
        }

        Ok(validator.voting_power())
    }

    /// Verify a round signature in a round certificate against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    async fn verify_round_signature(
        &self,
        ctx: &Ctx,
        certificate: &RoundCertificate<Ctx>,
        signature: &RoundSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        let vote_type = signature.vote_type;
        let vote = match vote_type {
            VoteType::Prevote => ctx.new_prevote(
                certificate.height,
                certificate.round,
                signature.value_id.clone(),
                validator.address().clone(),
            ),
            VoteType::Precommit => ctx.new_precommit(
                certificate.height,
                certificate.round,
                signature.value_id.clone(),
                validator.address().clone(),
            ),
        };

        // Verify signature
        if self
            .verify_signed_vote(&vote, &signature.signature, validator.public_key())
            .await
            .map_err(|e| CertificateError::VerificationError(e.into_source()))?
            .is_invalid()
        {
            return Err(CertificateError::InvalidRoundSignature(signature.clone()));
        }

        Ok(validator.voting_power())
    }

    /// Verify the commit certificate against the given validator set.
    ///
    /// - For each commit signature in the certificate:
    ///   - Reconstruct the signed precommit and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    async fn verify_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let mut signed_voting_power = 0;
        let mut seen_validators = Vec::new();

        // For each commit signature, reconstruct the signed precommit and verify the signature
        for commit_sig in &certificate.commit_signatures {
            let validator_address = &commit_sig.address;

            if seen_validators.contains(&validator_address) {
                return Err(CertificateError::DuplicateVote(validator_address.clone()));
            }

            seen_validators.push(validator_address);

            // Abort if validator not in validator set
            let validator = validator_set
                .get_by_address(validator_address)
                .ok_or_else(|| CertificateError::UnknownValidator(validator_address.clone()))?;

            if let Ok(voting_power) = self
                .verify_commit_signature(ctx, certificate, commit_sig, validator)
                .await
            {
                signed_voting_power += voting_power;
            }
        }

        let total_voting_power = validator_set.total_voting_power();

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

    /// Verify the polka certificate against the given validator set.
    ///
    /// - For each signed prevote in the certificate:
    ///   - Reconstruct the signed prevote and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    ///
    async fn verify_polka_certificate(
        &self,
        ctx: &Ctx,
        certificate: &PolkaCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let mut signed_voting_power = 0;
        let mut seen_validators = Vec::new();

        for signature in &certificate.polka_signatures {
            let validator_address = &signature.address;

            // Abort if validator already voted
            if seen_validators.contains(&validator_address) {
                return Err(CertificateError::DuplicateVote(validator_address.clone()));
            }

            // Add the validator to the list of seenv validators
            seen_validators.push(validator_address);

            // Abort if validator not in validator set
            let validator = validator_set
                .get_by_address(validator_address)
                .ok_or_else(|| CertificateError::UnknownValidator(validator_address.clone()))?;

            // Check that the vote signature is valid. Do this last and lazily as it is expensive.
            if let Ok(voting_power) = self
                .verify_polka_signature(ctx, certificate, signature, validator)
                .await
            {
                signed_voting_power += voting_power;
            }
        }

        let total_voting_power = validator_set.total_voting_power();

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

    /// Verify the round certificate against the given validator set.
    ///
    /// - For each signature in the certificate:
    ///   - Reconstruct the signed vote and verify its signature.
    /// - Check that the required voting power has signed the certificate:
    ///   - If `Precommit`, ensure that 2/3+ of the voting power is represented.
    ///   - If `Skip`, ensure that 1/3+ of the voting power is represented.
    ///  
    /// Returns a [`CertificateError`] if any verification step fails.
    async fn verify_round_certificate(
        &self,
        ctx: &Ctx,
        certificate: &RoundCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let mut signed_voting_power = 0;
        let mut seen_validators = Vec::new();

        for signature in &certificate.round_signatures {
            let validator_address = &signature.address;

            // Abort if validator already voted
            if seen_validators.contains(&validator_address) {
                return Err(CertificateError::DuplicateVote(validator_address.clone()));
            }

            // Add the validator to the list of seenv validators
            seen_validators.push(validator_address);

            // Abort if validator not in validator set
            let validator = validator_set
                .get_by_address(validator_address)
                .ok_or_else(|| CertificateError::UnknownValidator(validator_address.clone()))?;

            // Precommit certificates must not contain votes of type Prevote.
            if certificate.cert_type == RoundCertificateType::Precommit
                && signature.vote_type == VoteType::Prevote
            {
                return Err(CertificateError::InvalidVoteType(validator_address.clone()));
            }

            // Check that the vote signature is valid. Do this last and lazily as it is expensive.
            if let Ok(voting_power) = self
                .verify_round_signature(ctx, certificate, signature, validator)
                .await
            {
                signed_voting_power += voting_power;
            }
        }

        let total_voting_power = validator_set.total_voting_power();

        let threshold = match certificate.cert_type {
            RoundCertificateType::Precommit => &thresholds.quorum,
            RoundCertificateType::Skip => &thresholds.honest,
        };

        if threshold.is_met(signed_voting_power, total_voting_power) {
            Ok(())
        } else {
            Err(CertificateError::NotEnoughVotingPower {
                signed: signed_voting_power,
                total: total_voting_power,
                expected: threshold.min_expected(total_voting_power),
            })
        }
    }
}

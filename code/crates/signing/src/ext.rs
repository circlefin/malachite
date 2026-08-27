use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use async_trait::async_trait;
use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, Context, ExtendedCommitCertificate,
    ExtendedCommitSignature, NilOrVal, PolkaCertificate, PolkaSignature, Round, RoundCertificate,
    RoundCertificateType, RoundSignature, Signature, SignedExtension, ThresholdParams, Validator,
    ValidatorSet, ValueId, VoteExtensionPolicy, VoteExtensionScope, VoteType, VotingPower,
};

use crate::Verifier;

/// Extension trait providing additional certificate verification functionality.
///
/// This trait extends the base [`Verifier`] functionality with methods for verifying
/// certificates against validator sets. It is automatically implemented for any type
/// that implements [`Verifier`].
#[async_trait]
pub trait VerifierExt<Ctx>
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
    ///   - Reconstruct the signed precommit and verify its signature.
    ///   - If the signature is invalid, the entire certificate is rejected and
    ///     nothing is stored.
    /// - Check that we have 2/3+ of voting power has signed the certificate.
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    async fn verify_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>>;

    /// Verify the given extended commit certificate against the given validator set.
    ///
    /// In addition to the checks performed by [`Self::verify_commit_certificate`]
    /// — reconstructing each precommit, verifying its signature, and enforcing
    /// the 2/3+ voting-power quorum — this method enforces the supplied
    /// [`VoteExtensionPolicy`]. [`VoteExtensionPolicy::Disabled`] rejects any
    /// present extension; [`VoteExtensionPolicy::Required`] rejects missing
    /// extensions and verifies every present extension against the
    /// [`VoteExtensionScope`] formed by `(height, round, value_id,
    /// validator_address)`. A single unexpected, missing, or invalid extension
    /// causes the entire certificate to be rejected.
    async fn verify_extended_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &ExtendedCommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
        vote_extension_policy: VoteExtensionPolicy,
    ) -> Result<(), CertificateError<Ctx>>;

    /// Verify the polka certificate against the given validator set.
    ///
    /// - For each signature in the certificate:
    ///   - Reconstruct the signed prevote and verify its signature.
    ///   - If the signature is invalid, the entire certificate is rejected and
    ///     known-bad signatures must never be stored or re-broadcast.
    /// - Check that we have 2/3+ of voting power has signed the certificate.
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
    ///   - If the signature is invalid, the entire certificate is rejected and
    ///     known-bad signatures must never be replayed into the vote keeper
    ///     or re-broadcast in a locally-built certificate.
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

trait CommitSignatureEntry<Ctx>
where
    Ctx: Context,
{
    fn address(&self) -> &Ctx::Address;

    fn signature(&self) -> &Signature<Ctx>;

    fn extension(&self) -> Option<&SignedExtension<Ctx>>;
}

impl<Ctx> CommitSignatureEntry<Ctx> for CommitSignature<Ctx>
where
    Ctx: Context,
{
    fn address(&self) -> &Ctx::Address {
        &self.address
    }

    fn signature(&self) -> &Signature<Ctx> {
        &self.signature
    }

    fn extension(&self) -> Option<&SignedExtension<Ctx>> {
        None
    }
}

impl<Ctx> CommitSignatureEntry<Ctx> for ExtendedCommitSignature<Ctx>
where
    Ctx: Context,
{
    fn address(&self) -> &Ctx::Address {
        &self.address
    }

    fn signature(&self) -> &Signature<Ctx> {
        &self.signature
    }

    fn extension(&self) -> Option<&SignedExtension<Ctx>> {
        self.extension.as_ref()
    }
}

struct CommitSignatureVerification<'a, Ctx>
where
    Ctx: Context,
{
    ctx: &'a Ctx,
    height: Ctx::Height,
    round: Round,
    value_id: &'a ValueId<Ctx>,
    vote_extension_policy: VoteExtensionPolicy,
}

struct CommitVerification<'a, Ctx>
where
    Ctx: Context,
{
    signature: CommitSignatureVerification<'a, Ctx>,
    validator_set: &'a Ctx::ValidatorSet,
    thresholds: ThresholdParams,
}

async fn verify_commit_signature_entry<Ctx, P, S>(
    verifier: &P,
    verification: &CommitSignatureVerification<'_, Ctx>,
    signature: &S,
    validator: &Ctx::Validator,
) -> Result<VotingPower, CertificateError<Ctx>>
where
    Ctx: Context,
    P: Verifier<Ctx>,
    S: CommitSignatureEntry<Ctx> + Sync,
{
    let vote = verification.ctx.new_precommit(
        verification.height,
        verification.round,
        NilOrVal::Val(verification.value_id.clone()),
        validator.address().clone(),
    );

    if verifier
        .verify_signed_vote(&vote, signature.signature(), validator.public_key())
        .await
        .map_err(|e| CertificateError::VerificationError(e.into_source()))?
        .is_invalid()
    {
        return Err(CertificateError::InvalidCommitSignature(
            CommitSignature::new(signature.address().clone(), signature.signature().clone()),
        ));
    }

    let Some(signed_ext) = signature.extension() else {
        if verification.vote_extension_policy.is_required() {
            return Err(CertificateError::MissingVoteExtension(
                signature.address().clone(),
            ));
        }

        return Ok(validator.voting_power());
    };

    if verification.vote_extension_policy.is_disabled() {
        return Err(CertificateError::UnexpectedVoteExtension(
            signature.address().clone(),
        ));
    }

    let scope = VoteExtensionScope::new(
        verification.height,
        verification.round,
        verification.value_id.clone(),
        validator.address().clone(),
    );

    if verifier
        .verify_signed_vote_extension(
            &scope,
            &signed_ext.message,
            &signed_ext.signature,
            validator.public_key(),
        )
        .await
        .map_err(|e| CertificateError::VerificationError(e.into_source()))?
        .is_invalid()
    {
        return Err(CertificateError::InvalidVoteExtensionSignature(
            validator.address().clone(),
        ));
    }

    Ok(validator.voting_power())
}

async fn verify_commit_signature_entries<Ctx, P, S>(
    verifier: &P,
    verification: CommitVerification<'_, Ctx>,
    signatures: &[S],
) -> Result<(), CertificateError<Ctx>>
where
    Ctx: Context,
    P: Verifier<Ctx>,
    S: CommitSignatureEntry<Ctx> + Sync,
{
    let mut signed_voting_power: VotingPower = 0;
    let mut seen_validators = BTreeSet::new();

    for signature in signatures {
        let validator_address = signature.address();

        if !seen_validators.insert(validator_address) {
            return Err(CertificateError::DuplicateVote(validator_address.clone()));
        }

        let validator = verification
            .validator_set
            .get_by_address(validator_address)
            .ok_or_else(|| CertificateError::UnknownValidator(validator_address.clone()))?;

        let voting_power =
            verify_commit_signature_entry(verifier, &verification.signature, signature, validator)
                .await?;
        signed_voting_power = signed_voting_power.checked_add(voting_power).ok_or(
            CertificateError::VotingPowerOverflow {
                signed: signed_voting_power,
                added: voting_power,
            },
        )?;
    }

    let total_voting_power = verification.validator_set.total_voting_power();

    if verification
        .thresholds
        .quorum
        .is_met(signed_voting_power, total_voting_power)
    {
        Ok(())
    } else {
        Err(CertificateError::NotEnoughVotingPower {
            signed: signed_voting_power,
            total: total_voting_power,
            expected: verification
                .thresholds
                .quorum
                .min_expected(total_voting_power),
        })
    }
}

#[async_trait]
impl<Ctx, P> VerifierExt<Ctx> for P
where
    Ctx: Context,
    P: Verifier<Ctx>,
{
    async fn verify_commit_signature(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        verify_commit_signature_entry(
            self,
            &CommitSignatureVerification {
                ctx,
                height: certificate.height,
                round: certificate.round,
                value_id: &certificate.value_id,
                vote_extension_policy: VoteExtensionPolicy::Disabled,
            },
            commit_sig,
            validator,
        )
        .await
    }

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

    async fn verify_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &CommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        verify_commit_signature_entries(
            self,
            CommitVerification {
                signature: CommitSignatureVerification {
                    ctx,
                    height: certificate.height,
                    round: certificate.round,
                    value_id: &certificate.value_id,
                    vote_extension_policy: VoteExtensionPolicy::Disabled,
                },
                validator_set,
                thresholds,
            },
            &certificate.commit_signatures,
        )
        .await
    }

    async fn verify_extended_commit_certificate(
        &self,
        ctx: &Ctx,
        certificate: &ExtendedCommitCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
        vote_extension_policy: VoteExtensionPolicy,
    ) -> Result<(), CertificateError<Ctx>> {
        verify_commit_signature_entries(
            self,
            CommitVerification {
                signature: CommitSignatureVerification {
                    ctx,
                    height: certificate.height,
                    round: certificate.round,
                    value_id: &certificate.value_id,
                    vote_extension_policy,
                },
                validator_set,
                thresholds,
            },
            &certificate.commit_signatures,
        )
        .await
    }

    async fn verify_polka_certificate(
        &self,
        ctx: &Ctx,
        certificate: &PolkaCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let mut signed_voting_power: VotingPower = 0;
        let mut seen_validators = Vec::new();

        for signature in &certificate.polka_signatures {
            let validator_address = &signature.address;

            // Abort if validator already voted
            if seen_validators.contains(&validator_address) {
                return Err(CertificateError::DuplicateVote(validator_address.clone()));
            }

            // Add the validator to the list of seen validators
            seen_validators.push(validator_address);

            // Abort if validator not in validator set
            let validator = validator_set
                .get_by_address(validator_address)
                .ok_or_else(|| CertificateError::UnknownValidator(validator_address.clone()))?;

            // Verify the signature and propagate the verification error.
            let voting_power = self
                .verify_polka_signature(ctx, certificate, signature, validator)
                .await?;

            signed_voting_power = signed_voting_power.checked_add(voting_power).ok_or(
                CertificateError::VotingPowerOverflow {
                    signed: signed_voting_power,
                    added: voting_power,
                },
            )?;
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

    async fn verify_round_certificate(
        &self,
        ctx: &Ctx,
        certificate: &RoundCertificate<Ctx>,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let mut signed_voting_power: VotingPower = 0;
        let mut seen_validators = Vec::new();

        for signature in &certificate.round_signatures {
            let validator_address = &signature.address;

            // Abort if validator already voted
            if seen_validators.contains(&validator_address) {
                return Err(CertificateError::DuplicateVote(validator_address.clone()));
            }

            // Add the validator to the list of seen validators
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

            // Verify the signature and propagate the verification error.
            let voting_power = self
                .verify_round_signature(ctx, certificate, signature, validator)
                .await?;

            signed_voting_power = signed_voting_power.checked_add(voting_power).ok_or(
                CertificateError::VotingPowerOverflow {
                    signed: signed_voting_power,
                    added: voting_power,
                },
            )?;
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

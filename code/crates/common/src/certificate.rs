use core::fmt;

use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{
    Context, NilOrVal, Round, Signature, SignedExtension, SignedVote, ThresholdParams, Validator,
    ValidatorSet, ValueId, Vote, VoteType, VotingPower,
};

/// Represents a signature for a certificate, including the address and the signature itself.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitSignature<Ctx: Context> {
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
    /// Vote extension
    pub extension: Option<SignedExtension<Ctx>>,
}

impl<Ctx: Context> CommitSignature<Ctx> {
    /// Create a new `CommitSignature` from an address and a signature, with an optional extension.
    pub fn new(
        address: Ctx::Address,
        signature: Signature<Ctx>,
        extension: Option<SignedExtension<Ctx>>,
    ) -> Self {
        Self {
            address,
            signature,
            extension,
        }
    }
}

/// Aggregated signature.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct AggregatedSignature<Ctx: Context> {
    /// A collection of commit signatures.
    pub signatures: Vec<CommitSignature<Ctx>>,
}

impl<Ctx: Context> AggregatedSignature<Ctx> {
    /// Create a new `AggregatedSignature` from a vector of commit signatures.
    pub fn new(signatures: Vec<CommitSignature<Ctx>>) -> Self {
        Self { signatures }
    }
}

/// Represents a certificate containing the message (height, round, value_id) and an aggregated signature.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitCertificate<Ctx: Context> {
    /// The height of the certificate.
    pub height: Ctx::Height,
    /// The round number associated with the certificate.
    pub round: Round,
    /// The identifier for the value being certified.
    pub value_id: ValueId<Ctx>,
    /// A vector of signatures that make up the certificate.
    pub aggregated_signature: AggregatedSignature<Ctx>, // TODO - type in context
}

impl<Ctx: Context> CommitCertificate<Ctx> {
    /// Creates a new `CommitCertificate` from a vector of signed votes.
    pub fn new(
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        commits: Vec<SignedVote<Ctx>>,
    ) -> Self {
        // Collect all commit signatures from the signed votes
        let signatures = commits
            .into_iter()
            .filter(|vote| {
                matches!(vote.value(), NilOrVal::Val(id) if id == &value_id)
                    && vote.vote_type() == VoteType::Precommit
                    && vote.round() == round
                    && vote.height() == height
            })
            .map(|signed_vote| CommitSignature {
                address: signed_vote.validator_address().clone(),
                signature: signed_vote.signature,
                extension: signed_vote.message.extension().cloned(),
            })
            .collect();

        // Create the aggregated signature
        let aggregated_signature = AggregatedSignature::new(signatures);

        Self {
            height,
            round,
            value_id,
            aggregated_signature,
        }
    }

    /// Verify the certificate against the given validator set.
    ///
    /// - For each commit signature in the certificate:
    ///   - Reconstruct the signed precommit and verify its signature
    /// - Check that we have 2/3+ of voting power has signed the certificate
    ///
    /// If any of those steps fail, return a [`CertificateError`].
    ///
    /// TODO: Move to Context
    pub fn verify(
        &self,
        ctx: &Ctx,
        validator_set: &Ctx::ValidatorSet,
        thresholds: ThresholdParams,
    ) -> Result<(), CertificateError<Ctx>> {
        let total_voting_power = validator_set.total_voting_power();
        let mut signed_voting_power = 0;

        // For each commit signature, reconstruct the signed precommit and verify the signature
        for commit_sig in &self.aggregated_signature.signatures {
            // Abort if validator not in validator set
            let Some(validator) = validator_set.get_by_address(&commit_sig.address) else {
                return Err(CertificateError::UnknownValidator(commit_sig.clone()));
            };

            let voting_power = self.verify_commit_signature(ctx, commit_sig, validator)?;
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

    /// Verify a commit signature against the public key of its validator.
    ///
    /// ## Return
    /// Return the voting power of that validator if the signature is valid.
    fn verify_commit_signature(
        &self,
        ctx: &Ctx,
        commit_sig: &CommitSignature<Ctx>,
        validator: &Ctx::Validator,
    ) -> Result<VotingPower, CertificateError<Ctx>> {
        // Reconstruct the vote that was signed
        let vote = Ctx::new_precommit(
            self.height,
            self.round,
            NilOrVal::Val(self.value_id.clone()),
            validator.address().clone(),
        );

        // Verify signature
        if !ctx.verify_signed_vote(&vote, &commit_sig.signature, validator.public_key()) {
            return Err(CertificateError::InvalidSignature(commit_sig.clone()));
        }

        Ok(validator.voting_power())
    }
}

/// Represents an error that can occur when verifying a certificate.
#[derive_where(Clone, Debug)]
pub enum CertificateError<Ctx: Context> {
    /// One of the commit signature is invalid.
    InvalidSignature(CommitSignature<Ctx>),

    /// A validator in the certificate is not in the validator set.
    UnknownValidator(CommitSignature<Ctx>),

    /// Not enough voting power has signed the certificate.
    NotEnoughVotingPower {
        /// Signed voting power
        signed: VotingPower,
        /// Total voting power
        total: VotingPower,
        /// Expected voting power
        expected: VotingPower,
    },
}

impl<Ctx: Context> fmt::Display for CertificateError<Ctx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CertificateError::InvalidSignature(commit_sig) => {
                write!(f, "Invalid commit signature: {commit_sig:?}")
            }

            CertificateError::UnknownValidator(commit_sig) => {
                write!(
                    f,
                    "A validator in the certificate is not in the validator set: {commit_sig:?}"
                )
            }

            CertificateError::NotEnoughVotingPower {
                signed,
                total,
                expected,
            } => {
                write!(
                    f,
                    "Not enough voting power has signed the certificate: \
                     signed={signed}, total={total}, expected={expected}",
                )
            }
        }
    }
}

impl<Ctx: Context> core::error::Error for CertificateError<Ctx> {}

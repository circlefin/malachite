use alloc::{string::String, vec::Vec};
use derive_where::derive_where;
use thiserror::Error;

use crate::{
    Context, NilOrVal, Round, Signature, SignedVote, ValueId, Vote, VoteType, VotingPower,
};

/// Represents a signature for a certificate, including the address and the signature itself.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitSignature<Ctx: Context> {
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
}

impl<Ctx: Context> CommitSignature<Ctx> {
    /// Create a new `CommitSignature` from an address and a signature, with an optional extension.
    pub fn new(address: Ctx::Address, signature: Signature<Ctx>) -> Self {
        Self { address, signature }
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
        let commit_signatures = commits
            .into_iter()
            .filter(|vote| {
                matches!(vote.value(), NilOrVal::Val(id) if id == &value_id)
                    && vote.vote_type() == VoteType::Precommit
                    && vote.round() == round
                    && vote.height() == height
            })
            .map(|signed_vote| {
                CommitSignature::new(
                    signed_vote.validator_address().clone(),
                    signed_vote.signature,
                )
            })
            .collect();

        // Create the aggregated signature
        let aggregated_signature = AggregatedSignature::new(commit_signatures);

        Self {
            height,
            round,
            value_id,
            aggregated_signature,
        }
    }
}

/// Represents a certificate witnessing a Polka at a given height and round.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct PolkaCertificate<Ctx: Context> {
    /// The height at which a Polka was witnessed
    pub height: Ctx::Height,
    /// The round at which a Polka that was witnessed
    pub round: Round,
    /// The value that the Polka is for
    pub value_id: ValueId<Ctx>,
    /// The votes that make up the Polka
    pub votes: Vec<SignedVote<Ctx>>,
}

/// Represents an error that can occur when verifying a certificate.
#[derive_where(Clone, Debug)]
#[derive(Error)]
pub enum CertificateError<Ctx: Context> {
    /// The validator set was not found at the given height.
    #[error("Validator set not found at height {0}")]
    ValidatorSetNotFound(Ctx::Height),

    /// One of the commit signature is invalid.
    #[error("Invalid commit signature: {0:?}")]
    InvalidSignature(CommitSignature<Ctx>),

    /// A validator in the certificate is not in the validator set.
    #[error("A validator in the certificate is not in the validator set: {0:?}")]
    UnknownValidator(CommitSignature<Ctx>),

    /// Not enough voting power has signed the certificate.
    #[error(
        "Not enough voting power has signed the certificate: \
         signed={signed}, total={total}, expected={expected}"
    )]
    NotEnoughVotingPower {
        /// Signed voting power
        signed: VotingPower,
        /// Total voting power
        total: VotingPower,
        /// Expected voting power
        expected: VotingPower,
    },

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    ProcessingError(String),
}

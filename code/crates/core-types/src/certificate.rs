use alloc::vec::Vec;
use derive_where::derive_where;
use thiserror::Error;

use crate::{
    Context, NilOrVal, Round, Signature, SignedVote, ValueId, Vote, VoteType, VotingPower,
};

/// Represents a signature for a commit certificate, with the address of the validator that produced it.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitSignature<Ctx: Context> {
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
}

impl<Ctx: Context> CommitSignature<Ctx> {
    /// Create a new `CommitSignature` from an address and a signature.
    pub fn new(address: Ctx::Address, signature: Signature<Ctx>) -> Self {
        Self { address, signature }
    }
}

/// Represents a certificate containing the message (height, round, value_id) and the commit signatures.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitCertificate<Ctx: Context> {
    /// The height of the certificate.
    pub height: Ctx::Height,
    /// The round number associated with the certificate.
    pub round: Round,
    /// The identifier for the value being certified.
    pub value_id: ValueId<Ctx>,
    /// A vector of signatures that make up the certificate.
    pub commit_signatures: Vec<CommitSignature<Ctx>>,
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

        Self {
            height,
            round,
            value_id,
            commit_signatures,
        }
    }
}

/// Represents a signature for a polka certificate, with the address of the validator that produced it.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct PolkaSignature<Ctx: Context> {
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
}

impl<Ctx: Context> PolkaSignature<Ctx> {
    /// Create a new `CommitSignature` from an address and a signature.
    pub fn new(address: Ctx::Address, signature: Signature<Ctx>) -> Self {
        Self { address, signature }
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
    /// The signatures for the votes that make up the Polka
    pub polka_signatures: Vec<PolkaSignature<Ctx>>,
}

impl<Ctx: Context> PolkaCertificate<Ctx> {
    /// Creates a new `PolkaCertificate` from signed prevotes.
    pub fn new(
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        votes: Vec<SignedVote<Ctx>>,
    ) -> Self {
        // Collect all polka signatures from the signed votes
        let polka_signatures = votes
            .into_iter()
            .filter(|vote| {
                matches!(vote.value(), NilOrVal::Val(id) if id == &value_id)
                    && vote.vote_type() == VoteType::Prevote
                    && vote.round() == round
                    && vote.height() == height
            })
            .map(|signed_vote| {
                PolkaSignature::new(
                    signed_vote.validator_address().clone(),
                    signed_vote.signature,
                )
            })
            .collect();

        Self {
            height,
            round,
            value_id,
            polka_signatures,
        }
    }
}

/// Represents an error that can occur when verifying a certificate.
#[derive(Error)]
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum CertificateError<Ctx: Context> {
    /// One of the commit signature is invalid.
    #[error("Invalid commit signature: {0:?}")]
    InvalidCommitSignature(CommitSignature<Ctx>),

    /// One of the commit signature is invalid.
    #[error("Invalid polka signature: {0:?}")]
    InvalidPolkaSignature(PolkaSignature<Ctx>),

    /// A validator in the certificate is not in the validator set.
    #[error("A validator in the certificate is not in the validator set: {0:?}")]
    UnknownValidator(Ctx::Address),

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

    /// Multiple votes from the same validator.
    #[error("Multiple votes from the same validator: {0}")]
    DuplicateVote(Ctx::Address),
}

/// Represents a signature for a round certificate, with the address of the validator that produced it.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct RoundSignature<Ctx: Context> {
    /// The vote type
    pub vote_type: VoteType,
    /// The value id
    pub value_id: NilOrVal<ValueId<Ctx>>,
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
}

impl<Ctx: Context> RoundSignature<Ctx> {
    /// Create a new `CommitSignature` from an address and a signature.
    pub fn new(
        vote_type: VoteType,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
        signature: Signature<Ctx>,
    ) -> Self {
        Self {
            vote_type,
            value_id,
            address,
            signature,
        }
    }
}

/// Represents a certificate for entering a new round at a given height.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct RoundCertificate<Ctx: Context> {
    /// The height at which a Polka was witnessed
    pub height: Ctx::Height,
    /// The round at which a Polka that was witnessed
    pub round: Round,
    /// The signatures for the votes that make up the certificate
    pub round_signatures: Vec<RoundSignature<Ctx>>,
}

impl<Ctx: Context> RoundCertificate<Ctx> {
    /// Creates a new `RoundCertificate` from a vector of signed votes.
    pub fn new_from_votes(height: Ctx::Height, round: Round, votes: Vec<SignedVote<Ctx>>) -> Self {
        RoundCertificate {
            height,
            round,
            round_signatures: votes
                .into_iter()
                .map(|v| {
                    RoundSignature::new(
                        v.vote_type(),
                        v.value().clone(),
                        v.validator_address().clone(),
                        v.signature,
                    )
                })
                .collect(),
        }
    }
}

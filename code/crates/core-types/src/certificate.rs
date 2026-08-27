use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec::Vec;
use bytes::Bytes;
use derive_where::derive_where;
use malachitebft_peer::PeerId;
use thiserror::Error;

use crate::{
    BoxError, Context, NilOrVal, Round, Signature, SignedExtension, SignedVote, ValueId, Vote,
    VoteExtensions, VoteType, VotingPower,
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

/// A commit signature bundled with the (optional) vote extension that was attached
/// to the same precommit vote.
///
/// Both signatures are scoped to the same height/round/value/validator, so a verifier
/// holding an [`ExtendedCommitSignature`] can cryptographically prove that the
/// extension and the precommit signature were produced together by the same validator
/// for the same decision.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedCommitSignature<Ctx: Context> {
    /// Address of the validator that produced the signature.
    pub address: Ctx::Address,
    /// Signature over the precommit (same bytes as [`CommitSignature::signature`]).
    pub signature: Signature<Ctx>,
    /// Vote extension signed by the same validator, bound to the same precommit
    /// scope via [`VoteExtensionScope`](crate::VoteExtensionScope).
    /// `None` when the validator did not attach an extension, or when the
    /// caller rebuilds this certificate from the host API's parallel
    /// `(CommitCertificate, VoteExtensions)` shape and no matching extension is
    /// available for this validator.
    pub extension: Option<SignedExtension<Ctx>>,
}

impl<Ctx: Context> ExtendedCommitSignature<Ctx> {
    /// Create a new `ExtendedCommitSignature`.
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

/// A self-verifiable commit certificate that bundles precommit signatures together
/// with their (optional) vote extensions.
///
/// Unlike the pair `(CommitCertificate, VoteExtensions)`, this type makes the
/// link between each commit signature and its extension structural. Combined with
/// the [`VoteExtensionScope`](crate::VoteExtensionScope)-bound signing scheme,
/// holding an `ExtendedCommitCertificate` is enough to prove that a quorum of
/// validators both committed to a value at `(height, round, value_id)` and
/// attested any included extensions for exactly that decision.
///
/// A bare [`CommitCertificate`] can always be derived from an
/// `ExtendedCommitCertificate` via [`Self::trim_vote_extensions`].
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedCommitCertificate<Ctx: Context> {
    /// Height at which the value was decided.
    pub height: Ctx::Height,
    /// Round in which the value was decided.
    pub round: Round,
    /// Identifier of the decided value.
    pub value_id: ValueId<Ctx>,
    /// Per-validator commit signatures paired with their (optional) extensions,
    /// sorted by validator address.
    pub commit_signatures: Vec<ExtendedCommitSignature<Ctx>>,
}

impl<Ctx: Context> ExtendedCommitCertificate<Ctx> {
    /// Build an `ExtendedCommitCertificate` from precommit votes that may carry
    /// signed extensions.
    ///
    /// Filters votes that do not match the certificate's `(height, round,
    /// value_id, vote_type=Precommit)` scope, then takes ownership of each vote's
    /// extension to attach it to the corresponding signature entry.
    pub fn from_votes(
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
        commits: Vec<SignedVote<Ctx>>,
    ) -> Self {
        let mut signatures: Vec<_> = commits
            .into_iter()
            .filter(|vote| {
                matches!(vote.value(), NilOrVal::Val(id) if id == &value_id)
                    && vote.vote_type() == VoteType::Precommit
                    && vote.round() == round
                    && vote.height() == height
            })
            .map(|mut signed_vote| {
                let address = signed_vote.validator_address().clone();
                let extension = signed_vote.message.take_extension();
                ExtendedCommitSignature::new(address, signed_vote.signature, extension)
            })
            .collect();
        Self::sort_signatures_by_address(&mut signatures);

        Self {
            height,
            round,
            value_id,
            commit_signatures: signatures,
        }
    }

    /// Rebuild an `ExtendedCommitCertificate` from the parallel
    /// `(CommitCertificate, VoteExtensions)` shape that the host API exposes.
    ///
    /// Each extension is matched to its commit signature by the signing
    /// validator's address. Extensions whose address does not appear in the
    /// certificate are discarded (they cannot be bound to a precommit signature
    /// here, so they would not be self-verifiable anyway). Commit signatures
    /// for which no extension is supplied get `extension: None`.
    pub fn from_commit_certificate_and_extensions(
        certificate: CommitCertificate<Ctx>,
        extensions: VoteExtensions<Ctx>,
    ) -> Self {
        let mut ext_by_address: BTreeMap<Ctx::Address, SignedExtension<Ctx>> =
            extensions.extensions.into_iter().collect();

        let mut signatures: Vec<_> = certificate
            .commit_signatures
            .into_iter()
            .map(|sig| {
                let extension = ext_by_address.remove(&sig.address);
                ExtendedCommitSignature::new(sig.address, sig.signature, extension)
            })
            .collect();
        Self::sort_signatures_by_address(&mut signatures);

        Self {
            height: certificate.height,
            round: certificate.round,
            value_id: certificate.value_id,
            commit_signatures: signatures,
        }
    }

    fn sort_signatures_by_address(signatures: &mut [ExtendedCommitSignature<Ctx>]) {
        signatures.sort_by(|a, b| a.address.cmp(&b.address));
    }

    /// Project this extended certificate down to a bare [`CommitCertificate`],
    /// dropping the vote extensions.
    pub fn trim_vote_extensions(&self) -> CommitCertificate<Ctx> {
        CommitCertificate {
            height: self.height,
            round: self.round,
            value_id: self.value_id.clone(),
            commit_signatures: self
                .commit_signatures
                .iter()
                .map(|s| CommitSignature::new(s.address.clone(), s.signature.clone()))
                .collect(),
        }
    }

    /// Extract the vote extensions carried by this certificate as a standalone
    /// [`VoteExtensions`] view, preserving each extension's signing validator.
    pub fn vote_extensions(&self) -> VoteExtensions<Ctx> {
        let extensions = self
            .commit_signatures
            .iter()
            .filter_map(|s| {
                s.extension
                    .as_ref()
                    .map(|ext| (s.address.clone(), ext.clone()))
            })
            .collect();

        VoteExtensions::new(extensions)
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
#[derive_where(Debug, PartialEq)]
pub enum CertificateError<Ctx: Context> {
    /// One of the commit signatures is invalid.
    #[error("Invalid commit signature: {0:?}")]
    InvalidCommitSignature(CommitSignature<Ctx>),

    /// One of the commit signatures is invalid.
    #[error("Invalid polka signature: {0:?}")]
    InvalidPolkaSignature(PolkaSignature<Ctx>),

    /// One of the round signatures is invalid.
    #[error("Invalid round signature: {0:?}")]
    InvalidRoundSignature(RoundSignature<Ctx>),

    /// A vote extension carried by an [`ExtendedCommitCertificate`] has an
    /// invalid signature for its precommit scope.
    #[error("Invalid vote extension signature from validator: {0}")]
    InvalidVoteExtensionSignature(Ctx::Address),

    /// A commit signature in an [`ExtendedCommitCertificate`] is missing its
    /// required vote extension.
    #[error("Missing vote extension from validator: {0}")]
    MissingVoteExtension(Ctx::Address),

    /// A commit signature in an [`ExtendedCommitCertificate`] carries a vote
    /// extension at a height where extensions must be absent.
    #[error("Unexpected vote extension from validator: {0}")]
    UnexpectedVoteExtension(Ctx::Address),

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

    /// Signed voting power overflowed while verifying the certificate.
    #[error(
        "Signed voting power overflowed while verifying the certificate: \
         signed={signed}, added={added}"
    )]
    VotingPowerOverflow {
        /// Signed voting power accumulated before the overflow.
        signed: VotingPower,
        /// Voting power that would overflow the accumulator.
        added: VotingPower,
    },

    /// Multiple votes from the same validator.
    #[error("Multiple votes from the same validator: {0}")]
    DuplicateVote(Ctx::Address),

    /// A Prevote was incorrectly included in a Precommit round certificate.
    #[error("Prevote received in precommit round certificate from validator: {0}")]
    InvalidVoteType(Ctx::Address),

    /// An error occurred while verifying the certificate.
    #[error("Signature verification error: {}", .0.as_ref().map(|e| e.to_string()).unwrap_or_default())]
    VerificationError(Option<BoxError>),
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

/// Describes the type of a `RoundCertificate`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "borsh",
    derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize)
)]
pub enum RoundCertificateType {
    /// Composed of f+1 votes (e.g., SkipRound)
    Skip,
    /// Composed of 2f+1 Precommit votes from the previous round (e.g., PrecommitAny)
    Precommit,
}

/// Represents a certificate used to justify entering a new round at a given height.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct RoundCertificate<Ctx: Context> {
    /// The height at which a certificate was witnessed
    pub height: Ctx::Height,
    /// The round of the votes that made up the certificate
    pub round: Round,
    /// The type of the certificate
    pub cert_type: RoundCertificateType,
    /// The signatures for the votes that make up the certificate
    pub round_signatures: Vec<RoundSignature<Ctx>>,
}

impl<Ctx: Context> RoundCertificate<Ctx> {
    /// Creates a new `RoundCertificate` from a vector of signed votes.
    pub fn new_from_votes(
        height: Ctx::Height,
        round: Round,
        cert_type: RoundCertificateType,
        votes: Vec<SignedVote<Ctx>>,
    ) -> Self {
        RoundCertificate {
            height,
            round,
            cert_type,
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

/// Represents a local certificate that triggered or will trigger the start of a new round.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct EnterRoundCertificate<Ctx: Context> {
    /// The certificate that triggered or will trigger the start of a new round
    pub certificate: RoundCertificate<Ctx>,
    /// The round that will be entered due to the `RoundCertificate`.
    /// - If the certificate is `PrecommitAny`, it contains signatures from the previous round,
    ///   so `enter_round` will be one more than the round of those signatures.
    /// - If the certificate is `SkipRound`, it contains signatures from the round being entered,
    ///   so `enter_round` will be equal to the round of those signatures.
    pub enter_round: Round,
}

impl<Ctx: Context> EnterRoundCertificate<Ctx> {
    /// Creates a new `LocalRoundCertificate` from a vector of signed votes.
    pub fn new_from_votes(
        height: Ctx::Height,
        enter_round: Round,
        round: Round,
        cert_type: RoundCertificateType,
        votes: Vec<SignedVote<Ctx>>,
    ) -> Self {
        Self {
            certificate: RoundCertificate::new_from_votes(height, round, cert_type, votes),
            enter_round,
        }
    }

    /// Creates a new `EnterRoundCertificate` by lifting the precommit signatures
    /// of an existing `CommitCertificate`.
    ///
    /// The caller chooses how the resulting certificate is to be interpreted:
    /// - `cert_type = Skip` with `enter_round = certificate.round` justifies
    ///   skipping into the certificate's round.
    /// - `cert_type = Precommit` with `enter_round = certificate.round.increment()`
    ///   justifies advancing past the certificate's round on a precommit timeout.
    pub fn from_commit_certificate(
        certificate: &CommitCertificate<Ctx>,
        cert_type: RoundCertificateType,
        enter_round: Round,
    ) -> Self {
        let round_signatures = certificate
            .commit_signatures
            .iter()
            .map(|cs| {
                RoundSignature::new(
                    VoteType::Precommit,
                    NilOrVal::Val(certificate.value_id.clone()),
                    cs.address.clone(),
                    cs.signature.clone(),
                )
            })
            .collect();

        Self {
            certificate: RoundCertificate {
                height: certificate.height,
                round: certificate.round,
                cert_type,
                round_signatures,
            },
            enter_round,
        }
    }
}

/// Represents a response to a value request.
///
/// Carries an [`ExtendedCommitCertificate`] so that a node deciding via sync can
/// observe the vote extensions attached to the original precommits, not just
/// the bare commit signatures. Without this, the sync-recovered node has no
/// extensions for the synced height and cannot act as the proposer of the
/// next height when the application uses extensions for load-bearing data
/// (oracle aggregation, threshold attestations, bridge outputs, etc.).
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueResponse<Ctx: Context> {
    /// The peer that sent the value response
    pub peer: PeerId,
    /// The raw bytes of the value
    pub value_bytes: Bytes,
    /// The extended commit certificate proving the value was decided, bundling
    /// per-validator commit signatures with the vote extensions
    /// they attached if required to do so.
    pub certificate: ExtendedCommitCertificate<Ctx>,
}

impl<Ctx: Context> ValueResponse<Ctx> {
    /// Creates a new `ValueResponse` from the raw bytes of the value and the
    /// extended commit certificate.
    pub fn new(
        peer: PeerId,
        value_bytes: Bytes,
        certificate: ExtendedCommitCertificate<Ctx>,
    ) -> Self {
        Self {
            peer,
            value_bytes,
            certificate,
        }
    }
}

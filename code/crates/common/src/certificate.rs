use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{Context, Extension, NilOrVal, Round, Signature, SignedVote, ValueId, Vote, VoteType};

/// Represents a signature for a certificate, including the address and the signature itself.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct CommitSignature<Ctx: Context> {
    /// The address associated with the signature.
    pub address: Ctx::Address,
    /// The signature itself.
    pub signature: Signature<Ctx>,
    /// Vote extension
    /// TODO - add extension signature
    pub extension: Option<Extension>,
}

/// Aggregated signature.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct AggregatedSignature<Ctx: Context> {
    /// A collection of commit signatures.
    pub signatures: Vec<CommitSignature<Ctx>>,
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
        let aggregated_signature = AggregatedSignature { signatures };

        CommitCertificate {
            height,
            round,
            value_id,
            aggregated_signature,
        }
    }
}

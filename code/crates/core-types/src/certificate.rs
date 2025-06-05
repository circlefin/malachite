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
    /// One of the commit signatures is invalid.
    #[error("Invalid commit signature: {0:?}")]
    InvalidCommitSignature(CommitSignature<Ctx>),

    /// One of the commit signatures is invalid.
    #[error("Invalid polka signature: {0:?}")]
    InvalidPolkaSignature(PolkaSignature<Ctx>),

    /// One of the round signatures is invalid.
    #[error("Invalid round signature: {0:?}")]
    InvalidRoundSignature(RoundSignature<Ctx>),

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

    /// A Prevote was incorrectly included in a Precommit round certificate.
    #[error("Prevote received in precommit round certificate from validator: {0}")]
    InvalidVoteType(Ctx::Address),
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
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
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
}

#[cfg(feature = "borsh")]
mod _borsh {
    use super::*;

    impl<Ctx: Context> borsh::BorshSerialize for PolkaSignature<Ctx>
    where
        Ctx::Address: borsh::BorshSerialize,
        Signature<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.address.serialize(writer)?;
            self.signature.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for PolkaSignature<Ctx>
    where
        Ctx::Address: borsh::BorshDeserialize,
        Signature<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let address = Ctx::Address::deserialize_reader(reader)?;
            let signature = Signature::<Ctx>::deserialize_reader(reader)?;
            Ok(PolkaSignature { address, signature })
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for PolkaCertificate<Ctx>
    where
        Ctx::Address: borsh::BorshSerialize,
        Ctx::Height: borsh::BorshSerialize,
        Signature<Ctx>: borsh::BorshSerialize,
        ValueId<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.height.serialize(writer)?;
            self.round.serialize(writer)?;
            self.value_id.serialize(writer)?;
            self.polka_signatures.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for PolkaCertificate<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
        Ctx::Address: borsh::BorshDeserialize,
        Signature<Ctx>: borsh::BorshDeserialize,
        ValueId<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height = Ctx::Height::deserialize_reader(reader)?;
            let round = Round::deserialize_reader(reader)?;
            let value_id = ValueId::<Ctx>::deserialize_reader(reader)?;
            let polka_signatures = Vec::<PolkaSignature<Ctx>>::deserialize_reader(reader)?;
            Ok(PolkaCertificate {
                height,
                round,
                value_id,
                polka_signatures,
            })
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for RoundCertificate<Ctx>
    where
        Ctx::Height: borsh::BorshSerialize,
        Ctx::Address: borsh::BorshSerialize,
        Signature<Ctx>: borsh::BorshSerialize,
        RoundSignature<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.height.serialize(writer)?;
            self.round.serialize(writer)?;
            self.cert_type.serialize(writer)?;
            self.round_signatures.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for RoundCertificate<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
        Ctx::Address: borsh::BorshDeserialize,
        Signature<Ctx>: borsh::BorshDeserialize,
        RoundSignature<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height = Ctx::Height::deserialize_reader(reader)?;
            let round = Round::deserialize_reader(reader)?;
            let cert_type = RoundCertificateType::deserialize_reader(reader)?;
            let round_signatures = Vec::<RoundSignature<Ctx>>::deserialize_reader(reader)?;
            Ok(RoundCertificate {
                height,
                round,
                cert_type,
                round_signatures,
            })
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for RoundSignature<Ctx>
    where
        NilOrVal<ValueId<Ctx>>: borsh::BorshSerialize,
        Ctx::Address: borsh::BorshSerialize,
        Signature<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.vote_type.serialize(writer)?;
            self.value_id.serialize(writer)?;
            self.address.serialize(writer)?;
            self.signature.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for RoundSignature<Ctx>
    where
        NilOrVal<ValueId<Ctx>>: borsh::BorshDeserialize,
        Ctx::Address: borsh::BorshDeserialize,
        Signature<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let vote_type = VoteType::deserialize_reader(reader)?;
            let value_id = NilOrVal::<ValueId<Ctx>>::deserialize_reader(reader)?;
            let address = Ctx::Address::deserialize_reader(reader)?;
            let signature = Signature::<Ctx>::deserialize_reader(reader)?;
            Ok(RoundSignature {
                vote_type,
                value_id,
                address,
                signature,
            })
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for CommitCertificate<Ctx>
    where
        Ctx::Height: borsh::BorshSerialize,
        ValueId<Ctx>: borsh::BorshSerialize,
        CommitSignature<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.height.serialize(writer)?;
            self.round.serialize(writer)?;
            self.value_id.serialize(writer)?;
            self.commit_signatures.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for CommitCertificate<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
        ValueId<Ctx>: borsh::BorshDeserialize,
        CommitSignature<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height = Ctx::Height::deserialize_reader(reader)?;
            let round = Round::deserialize_reader(reader)?;
            let value_id = ValueId::<Ctx>::deserialize_reader(reader)?;
            let commit_signatures = Vec::<CommitSignature<Ctx>>::deserialize_reader(reader)?;
            Ok(CommitCertificate {
                height,
                round,
                value_id,
                commit_signatures,
            })
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for CommitSignature<Ctx>
    where
        Ctx::Address: borsh::BorshSerialize,
        Signature<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.address.serialize(writer)?;
            self.signature.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for CommitSignature<Ctx>
    where
        Ctx::Address: borsh::BorshDeserialize,
        Signature<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let address = Ctx::Address::deserialize_reader(reader)?;
            let signature = Signature::<Ctx>::deserialize_reader(reader)?;
            Ok(CommitSignature { address, signature })
        }
    }
}

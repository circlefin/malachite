use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::{
    Context, PolkaCertificate, Proposal, Round, RoundCertificate, Signature, SignedProposal,
    SignedVote, Timeout, Validity, Vote,
};

pub use malachitebft_core_types::ValuePayload;

pub use malachitebft_peer::PeerId;
pub use multiaddr::Multiaddr;

/// The role that the node is playing in the consensus protocol during a round.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// The node is the proposer for the current round.
    Proposer,
    /// The node is a validator for the current round.
    Validator,
    /// The node is not participating in the consensus protocol for the current round.
    None,
}

/// A signed consensus message, ie. a signed vote or a signed proposal.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum SignedConsensusMsg<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    Proposal(SignedProposal<Ctx>),
}

impl<Ctx: Context> SignedConsensusMsg<Ctx> {
    pub fn height(&self) -> Ctx::Height {
        match self {
            SignedConsensusMsg::Vote(msg) => msg.height(),
            SignedConsensusMsg::Proposal(msg) => msg.height(),
        }
    }

    pub fn round(&self) -> Round {
        match self {
            SignedConsensusMsg::Vote(msg) => msg.round(),
            SignedConsensusMsg::Proposal(msg) => msg.round(),
        }
    }

    pub fn signature(&self) -> &Signature<Ctx> {
        match self {
            SignedConsensusMsg::Vote(msg) => &msg.signature,
            SignedConsensusMsg::Proposal(msg) => &msg.signature,
        }
    }
}

/// A message that can be sent by the consensus layer
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusMsg<Ctx: Context> {
    Vote(Ctx::Vote),
    Proposal(Ctx::Proposal),
}

/// A value to propose by the current node.
/// Used only when the node is the proposer.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Ctx::Value,
}

impl<Ctx: Context> LocallyProposedValue<Ctx> {
    pub fn new(height: Ctx::Height, round: Round, value: Ctx::Value) -> Self {
        Self {
            height,
            round,
            value,
        }
    }
}

/// A value proposed by a validator
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub valid_round: Round,
    pub proposer: Ctx::Address,
    pub value: Ctx::Value,
    pub validity: Validity,
}

#[derive_where(Clone, Debug)]
pub enum WalEntry<Ctx: Context> {
    ConsensusMsg(SignedConsensusMsg<Ctx>),
    Timeout(Timeout),
    ProposedValue(ProposedValue<Ctx>),
}

impl<Ctx: Context> WalEntry<Ctx> {
    pub fn as_consensus_msg(&self) -> Option<&SignedConsensusMsg<Ctx>> {
        match self {
            WalEntry::ConsensusMsg(msg) => Some(msg),
            _ => None,
        }
    }

    pub fn as_timeout(&self) -> Option<&Timeout> {
        match self {
            WalEntry::Timeout(timeout) => Some(timeout),
            _ => None,
        }
    }

    pub fn as_proposed_value(&self) -> Option<&ProposedValue<Ctx>> {
        match self {
            WalEntry::ProposedValue(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum VoteExtensionError {
    #[error("Invalid vote extension signature")]
    InvalidSignature,
    #[error("Invalid vote extension")]
    InvalidVoteExtension,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum LivenessMsg<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    PolkaCertificate(PolkaCertificate<Ctx>),
    SkipRoundCertificate(RoundCertificate<Ctx>),
}

#[cfg(feature = "borsh")]
mod _borsh {
    use super::*;

    impl<Ctx: Context> borsh::BorshSerialize for SignedConsensusMsg<Ctx>
    where
        SignedVote<Ctx>: borsh::BorshSerialize,
        SignedProposal<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            match self {
                SignedConsensusMsg::Vote(signed_message) => {
                    0u8.serialize(writer)?;
                    signed_message.serialize(writer)
                }
                SignedConsensusMsg::Proposal(signed_message) => {
                    1u8.serialize(writer)?;
                    signed_message.serialize(writer)
                }
            }
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for SignedConsensusMsg<Ctx>
    where
        SignedVote<Ctx>: borsh::BorshDeserialize,
        SignedProposal<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let discriminant = u8::deserialize_reader(reader)?;
            match discriminant {
                0 => Ok(SignedConsensusMsg::Vote(SignedVote::deserialize_reader(
                    reader,
                )?)),
                1 => Ok(SignedConsensusMsg::Proposal(
                    SignedProposal::deserialize_reader(reader)?,
                )),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid discriminant",
                )),
            }
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for LivenessMsg<Ctx>
    where
        SignedVote<Ctx>: borsh::BorshSerialize,
        PolkaCertificate<Ctx>: borsh::BorshSerialize,
        RoundCertificate<Ctx>: borsh::BorshSerialize,
    {
        fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
            match self {
                LivenessMsg::Vote(signed_message) => {
                    0u8.serialize(writer)?;
                    signed_message.serialize(writer)
                }
                LivenessMsg::PolkaCertificate(polka_certificate) => {
                    1u8.serialize(writer)?;
                    polka_certificate.serialize(writer)
                }
                LivenessMsg::SkipRoundCertificate(round_certificate) => {
                    2u8.serialize(writer)?;
                    round_certificate.serialize(writer)
                }
            }
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for LivenessMsg<Ctx>
    where
        SignedVote<Ctx>: borsh::BorshDeserialize,
        PolkaCertificate<Ctx>: borsh::BorshDeserialize,
        RoundCertificate<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let discriminant = u8::deserialize_reader(reader)?;
            match discriminant {
                0 => Ok(LivenessMsg::Vote(SignedVote::deserialize_reader(reader)?)),
                1 => Ok(LivenessMsg::PolkaCertificate(
                    PolkaCertificate::deserialize_reader(reader)?,
                )),
                2 => Ok(LivenessMsg::SkipRoundCertificate(
                    RoundCertificate::deserialize_reader(reader)?,
                )),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid discriminant",
                )),
            }
        }
    }

    impl<Ctx: Context> borsh::BorshSerialize for ProposedValue<Ctx>
    where
        Ctx::Height: borsh::BorshSerialize,
        Ctx::Address: borsh::BorshSerialize,
        Ctx::Value: borsh::BorshSerialize,
    {
        fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
            self.height.serialize(writer)?;
            self.round.serialize(writer)?;
            self.valid_round.serialize(writer)?;
            self.proposer.serialize(writer)?;
            self.value.serialize(writer)?;
            self.validity.serialize(writer)
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for ProposedValue<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
        Ctx::Address: borsh::BorshDeserialize,
        Ctx::Value: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height = Ctx::Height::deserialize_reader(reader)?;
            let round = Round::deserialize_reader(reader)?;
            let valid_round = Round::deserialize_reader(reader)?;
            let proposer = Ctx::Address::deserialize_reader(reader)?;
            let value = Ctx::Value::deserialize_reader(reader)?;
            let validity = Validity::deserialize_reader(reader)?;
            Ok(ProposedValue {
                height,
                round,
                valid_round,
                proposer,
                value,
                validity,
            })
        }
    }
}

use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::{
    Context, PolkaCertificate, Proposal, Round, RoundCertificate, Signature, SignedProposal,
    SignedVote, Timeout, Validity, Vote,
};

pub use malachitebft_core_types::ValuePayload;

pub use malachitebft_peer::PeerId;
pub use multiaddr::Multiaddr;

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

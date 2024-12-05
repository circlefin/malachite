use derive_where::derive_where;

use malachite_common::{
    Context, Proposal, Round, SignedExtension, SignedProposal, SignedVote, Validity, Vote,
};

pub use malachite_peer::PeerId;
pub use multiaddr::Multiaddr;

/// A signed consensus message, ie. a signed vote or a signed proposal.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum SignedConsensusMsg<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    Proposal(SignedProposal<Ctx>),
}

impl<Ctx: Context> SignedConsensusMsg<Ctx> {
    pub fn msg_height(&self) -> Ctx::Height {
        match self {
            SignedConsensusMsg::Vote(msg) => msg.height(),
            SignedConsensusMsg::Proposal(msg) => msg.height(),
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
pub struct ValueToPropose<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub valid_round: Round,
    pub value: Ctx::Value,
    pub extension: Option<SignedExtension<Ctx>>,
}

/// A value proposed by a validator
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub valid_round: Round,
    pub validator_address: Ctx::Address,
    pub value: Ctx::Value,
    pub validity: Validity,
    pub extension: Option<SignedExtension<Ctx>>,
}

/// The possible messages used to deliver proposals
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValuePayload {
    PartsOnly,
    ProposalOnly,
    ProposalAndParts,
}

impl ValuePayload {
    pub fn include_proposal(self) -> bool {
        matches!(
            self,
            ValuePayload::ProposalOnly | ValuePayload::ProposalAndParts
        )
    }

    pub fn include_parts(self) -> bool {
        matches!(
            self,
            ValuePayload::PartsOnly | ValuePayload::ProposalAndParts
        )
    }

    pub fn parts_only(self) -> bool {
        matches!(self, ValuePayload::PartsOnly)
    }

    pub fn proposal_only(&self) -> bool {
        matches!(self, ValuePayload::ProposalOnly)
    }
}

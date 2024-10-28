use derive_where::derive_where;

use malachite_common::{
    Context, Extension, Proposal, Round, SignedProposal, SignedVote, Validity, Vote,
};

pub use libp2p_identity::PeerId;
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

/// A value proposed by a validator
/// Called at non-proposer only.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub valid_round: Round,
    pub validator_address: Ctx::Address,
    pub value: Ctx::Value,
    pub validity: Validity,
    pub extension: Option<Extension>,
}

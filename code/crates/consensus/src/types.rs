use derive_where::derive_where;
use malachite_common::{
    Context, Proposal, ProposalPart, Round, SignedProposal, SignedProposalPart, SignedVote,
    Validity, Vote,
};

pub use libp2p_identity::PeerId;
pub use multiaddr::Multiaddr;

/// A message that can be broadcast by the gossip layer
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum GossipMsg<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    Proposal(SignedProposal<Ctx>),
    ProposalPart(SignedProposalPart<Ctx>),
}

/// A signed consensus message.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum SignedMessage<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    Proposal(SignedProposal<Ctx>),
    ProposalPart(SignedProposalPart<Ctx>),
}

impl<Ctx: Context> GossipMsg<Ctx> {
    pub fn msg_height(&self) -> Option<Ctx::Height> {
        match self {
            GossipMsg::Vote(msg) => Some(msg.vote.height()),
            GossipMsg::Proposal(msg) => Some(msg.proposal.height()),
            GossipMsg::ProposalPart(msg) => Some(msg.proposal_part.height()),
        }
    }
}

/// An event that can be emitted by the gossip layer
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum GossipEvent<Ctx: Context> {
    Listening(Multiaddr),
    Message(PeerId, GossipMsg<Ctx>),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

/// A value proposed by a validator
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub validator_address: Ctx::Address,
    pub value: Ctx::Value,
    pub validity: Validity,
}

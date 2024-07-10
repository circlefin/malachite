use core::fmt;
use std::marker::PhantomData;

use derive_where::derive_where;
use malachite_common::{
    BlockPart, Context, Proposal, SignedBlockPart, SignedProposal, SignedVote, Vote,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PeerId;

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Multiaddr;

impl fmt::Display for Multiaddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Multiaddr")
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    Consensus,
    BlockParts,
}

#[derive_where(Clone, Debug, PartialEq)]
pub enum NetworkMsg<Ctx: Context> {
    Vote(SignedVote<Ctx>),
    Proposal(SignedProposal<Ctx>),
    BlockPart(SignedBlockPart<Ctx>),
}

impl<Ctx: Context> NetworkMsg<Ctx> {
    pub fn channel(&self) -> Channel {
        match self {
            NetworkMsg::Vote(_) | NetworkMsg::Proposal(_) => Channel::Consensus,
            NetworkMsg::BlockPart(_) => Channel::BlockParts,
        }
    }

    pub fn msg_height(&self) -> Option<Ctx::Height> {
        match self {
            NetworkMsg::Vote(msg) => Some(msg.vote.height()),
            NetworkMsg::Proposal(msg) => Some(msg.proposal.height()),
            NetworkMsg::BlockPart(msg) => Some(msg.block_part.height()),
        }
    }
}

#[derive_where(Debug)]
pub enum GossipEvent<Ctx: Context> {
    Listening(Multiaddr),
    Message(PeerId, NetworkMsg<Ctx>),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

#[derive_where(Debug)]
pub struct ReceivedProposedValue<Ctx> {
    marker: PhantomData<Ctx>,
}

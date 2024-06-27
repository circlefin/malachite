use derive_where::derive_where;
use prost::Name;
use prost_types::Any;

use malachite_common::{
    BlockPart, Context, Proposal, SignedBlockPart, SignedProposal, SignedVote, Vote,
};
use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;

use crate::Channel;

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

    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::to_bytes(self)
    }

    pub fn msg_height(&self) -> Option<Ctx::Height> {
        match self {
            NetworkMsg::Vote(msg) => Some(msg.vote.height()),
            NetworkMsg::Proposal(msg) => Some(msg.proposal.height()),
            NetworkMsg::BlockPart(msg) => Some(msg.block_part.height()),
        }
    }
}

impl<Ctx: Context> Protobuf for NetworkMsg<Ctx>
where
    SignedVote<Ctx>: Protobuf,
    SignedProposal<Ctx>: Protobuf,
    SignedBlockPart<Ctx>: Protobuf,
{
    type Proto = Any;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.type_url == <SignedVote<Ctx> as Protobuf>::Proto::type_url() {
            let vote = SignedVote::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(NetworkMsg::Vote(vote))
        } else if proto.type_url == <SignedProposal<Ctx> as Protobuf>::Proto::type_url() {
            let proposal = SignedProposal::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(NetworkMsg::Proposal(proposal))
        } else if proto.type_url == <SignedBlockPart<Ctx> as Protobuf>::Proto::type_url() {
            let block_part = SignedBlockPart::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(NetworkMsg::BlockPart(block_part))
        } else {
            Err(ProtoError::UnknownMessageType {
                type_url: proto.type_url,
            })
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        match self {
            NetworkMsg::Vote(vote) => vote.to_any(),
            NetworkMsg::Proposal(proposal) => proposal.to_any(),
            NetworkMsg::BlockPart(block_part) => block_part.to_any(),
        }
    }
}

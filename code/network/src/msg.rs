use prost::{Message, Name};
use prost_types::Any;

use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;
use malachite_proto::{SignedBlockPart, SignedProposal, SignedVote};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Vote(SignedVote),
    Proposal(SignedProposal),
    BlockPart(SignedBlockPart),
}

impl Msg {
    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::to_bytes(self)
    }

    pub fn msg_height(&self) -> Option<u64> {
        match self {
            Msg::Vote(msg) => Some(msg.vote.as_ref()?.height.as_ref()?.value),
            Msg::Proposal(msg) => Some(msg.proposal.as_ref()?.height.as_ref()?.value),
            Msg::BlockPart(msg) => Some(msg.block_part.as_ref()?.height.as_ref()?.value),
        }
    }
}

impl Protobuf for Msg {
    type Proto = Any;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.type_url == SignedVote::type_url() {
            let vote = SignedVote::decode(proto.value.as_slice())?;
            Ok(Msg::Vote(vote))
        } else if proto.type_url == SignedProposal::type_url() {
            let proposal = SignedProposal::decode(proto.value.as_slice())?;
            Ok(Msg::Proposal(proposal))
        } else if proto.type_url == SignedBlockPart::type_url() {
            let block_part = SignedBlockPart::decode(proto.value.as_slice())?;
            Ok(Msg::BlockPart(block_part))
        } else {
            Err(ProtoError::UnknownMessageType {
                type_url: proto.type_url,
            })
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(match self {
            Msg::Vote(vote) => Any {
                type_url: SignedVote::type_url(),
                value: vote.encode_to_vec(),
            },
            Msg::Proposal(proposal) => Any {
                type_url: SignedProposal::type_url(),
                value: proposal.encode_to_vec(),
            },
            Msg::BlockPart(block_part) => Any {
                type_url: SignedBlockPart::type_url(),
                value: block_part.encode_to_vec(),
            },
        })
    }
}

use std::ops::Deref;

use derive_where::derive_where;
use prost::Name;
use prost_types::Any;

use malachite_common::{Context, SignedProposal, SignedProposalPart, SignedVote};
use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;

pub use malachite_consensus::GossipMsg;

#[derive_where(Clone, Debug, PartialEq)]
pub struct NetworkMsg<Ctx: Context>(pub GossipMsg<Ctx>);

impl<Ctx: Context> Deref for NetworkMsg<Ctx> {
    type Target = GossipMsg<Ctx>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Ctx: Context> NetworkMsg<Ctx> {
    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::to_bytes(self)
    }
}

impl<Ctx: Context> Protobuf for NetworkMsg<Ctx>
where
    SignedVote<Ctx>: Protobuf,
    SignedProposal<Ctx>: Protobuf,
    SignedProposalPart<Ctx>: Protobuf,
{
    type Proto = Any;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.type_url == <SignedVote<Ctx> as Protobuf>::Proto::type_url() {
            let vote = SignedVote::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(Self(GossipMsg::Vote(vote)))
        } else if proto.type_url == <SignedProposal<Ctx> as Protobuf>::Proto::type_url() {
            let proposal = SignedProposal::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(Self(GossipMsg::Proposal(proposal)))
        } else if proto.type_url == <SignedProposalPart<Ctx> as Protobuf>::Proto::type_url() {
            let proposal_part = SignedProposalPart::<Ctx>::from_bytes(proto.value.as_slice())?;
            Ok(Self(GossipMsg::ProposalPart(proposal_part)))
        } else {
            Err(ProtoError::UnknownMessageType {
                type_url: proto.type_url,
            })
        }
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        match &self.0 {
            GossipMsg::Vote(vote) => vote.to_any(),
            GossipMsg::Proposal(proposal) => proposal.to_any(),
            GossipMsg::ProposalPart(proposal_part) => proposal_part.to_any(),
        }
    }
}

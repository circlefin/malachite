use prost::{Message, Name};
use prost_types::Any;

use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;
use malachite_proto::{SignedProposal, SignedVote};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Vote(SignedVote),
    Proposal(SignedProposal),

    #[cfg(test)]
    Dummy(u64),
}

impl Msg {
    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::to_bytes(self)
    }

    const DUMMY_TYPE_URL: &'static str = "malachite.Dummy";
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
        } else if cfg!(test) && proto.type_url == Msg::DUMMY_TYPE_URL {
            #[cfg(test)]
            {
                let value = u64::from_be_bytes(proto.value.try_into().unwrap());
                Ok(Msg::Dummy(value))
            }

            #[cfg(not(test))]
            {
                Err(ProtoError::UnknownMessageType {
                    type_url: Msg::DUMMY_TYPE_URL.to_string(),
                })
            }
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

            #[cfg(test)]
            Msg::Dummy(value) => Any {
                type_url: Msg::DUMMY_TYPE_URL.to_string(),
                value: value.to_be_bytes().to_vec(),
            },
        })
    }
}

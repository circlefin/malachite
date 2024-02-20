use prost::Message;
use prost::Name;
use prost_types::Any;

use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;
use malachite_proto::{Proposal, SignedVote};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Vote(SignedVote),
    Proposal(Proposal),

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

impl From<Msg> for Any {
    fn from(msg: Msg) -> Self {
        match msg {
            Msg::Vote(vote) => Any {
                type_url: SignedVote::type_url(),
                value: vote.encode_to_vec(),
            },
            Msg::Proposal(proposal) => Any {
                type_url: Proposal::type_url(),
                value: proposal.encode_to_vec(),
            },

            #[cfg(test)]
            Msg::Dummy(value) => Any {
                type_url: Msg::DUMMY_TYPE_URL.to_string(),
                value: value.to_be_bytes().to_vec(),
            },
        }
    }
}

impl TryFrom<Any> for Msg {
    type Error = ProtoError;

    fn try_from(any: Any) -> Result<Self, Self::Error> {
        if any.type_url == SignedVote::type_url() {
            let vote = SignedVote::decode(any.value.as_slice())?;
            Ok(Msg::Vote(vote))
        } else if any.type_url == Proposal::type_url() {
            let proposal = Proposal::decode(any.value.as_slice())?;
            Ok(Msg::Proposal(proposal))
        } else if cfg!(test) && any.type_url == Msg::DUMMY_TYPE_URL {
            #[cfg(test)]
            {
                let value = u64::from_be_bytes(any.value.try_into().unwrap());
                Ok(Msg::Dummy(value))
            }

            #[cfg(not(test))]
            {
                Err(malachite_proto::Error::Other(format!(
                    "unknown message type: {}",
                    Msg::DUMMY_TYPE_URL
                )))
            }
        } else {
            Err(ProtoError::Other(format!(
                "unknown message type: {}",
                any.type_url
            )))
        }
    }
}

impl Protobuf for Msg {
    type Proto = Any;

    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoError>
    where
        Self: Sized,
    {
        let any = Any::decode(bytes)?;
        Self::try_from(any)
    }

    fn into_bytes(self) -> Result<Vec<u8>, ProtoError> {
        Ok(Any::from(self).encode_to_vec())
    }
}

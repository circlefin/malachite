use prost::Message;
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
        Protobuf::<Any>::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::<Any>::to_bytes(self)
    }

    const DUMMY_TYPE_URL: &'static str = "malachite.Dummy";
}

impl Protobuf<Any> for Msg {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ProtoError>
    where
        Self: Sized,
    {
        use prost::Name;

        let any = Any::decode(bytes)?;

        if any.type_url == SignedVote::type_url() {
            let vote = SignedVote::decode(any.value.as_slice())?;
            Ok(Msg::Vote(vote))
        } else if any.type_url == malachite_proto::Proposal::type_url() {
            let proposal = Proposal::decode(any.value.as_slice())?;
            Ok(Msg::Proposal(proposal))
        } else if any.type_url == Msg::DUMMY_TYPE_URL {
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

    fn into_bytes(self) -> Result<Vec<u8>, ProtoError> {
        match self {
            Msg::Vote(vote) => {
                let any = Any::from_msg(&vote)?;
                Ok(any.encode_to_vec())
            }
            Msg::Proposal(proposal) => {
                let any = Any::from_msg(&proposal)?;
                Ok(any.encode_to_vec())
            }

            #[cfg(test)]
            Msg::Dummy(value) => {
                let any = Any {
                    type_url: Msg::DUMMY_TYPE_URL.to_string(),
                    value: value.to_be_bytes().to_vec(),
                };

                Ok(any.encode_to_vec())
            }
        }
    }
}

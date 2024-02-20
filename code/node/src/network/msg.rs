use derive_where::derive_where;

use prost::Message;
use prost_types::Any;

use malachite_common::Context;
use malachite_proto::Error as ProtoError;
use malachite_proto::Protobuf;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Msg<Ctx: Context> {
    Vote(Ctx::Vote),
    Proposal(Ctx::Proposal),

    #[cfg(test)]
    Dummy(u64),
}

impl<Ctx: Context> Msg<Ctx> {
    pub fn from_network_bytes(bytes: &[u8]) -> Result<Self, ProtoError> {
        Protobuf::<Any>::from_bytes(bytes)
    }

    pub fn to_network_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        Protobuf::<Any>::to_bytes(self)
    }
}

impl<Ctx: Context> Protobuf<Any> for Msg<Ctx> {
    fn from_bytes(bytes: &[u8]) -> Result<Self, malachite_proto::Error>
    where
        Self: Sized,
    {
        use prost::Name;

        let any = Any::decode(bytes)?;

        if any.type_url == malachite_proto::Vote::type_url() {
            let vote = Ctx::Vote::from_bytes(&any.value)?;
            Ok(Msg::Vote(vote))
        } else if any.type_url == malachite_proto::Proposal::type_url() {
            let proposal = Ctx::Proposal::from_bytes(&any.value)?;
            Ok(Msg::Proposal(proposal))
        } else if any.type_url == "malachite.proto.Dummy" {
            #[cfg(test)]
            {
                let value = u64::from_be_bytes(any.value.try_into().unwrap());
                Ok(Msg::Dummy(value))
            }

            #[cfg(not(test))]
            {
                Err(malachite_proto::Error::Other(
                    "unknown message type: malachite.proto.Dummy".to_string(),
                ))
            }
        } else {
            Err(malachite_proto::Error::Other(format!(
                "unknown message type: {}",
                any.type_url
            )))
        }
    }

    fn into_bytes(self) -> Result<Vec<u8>, malachite_proto::Error> {
        use prost::Name;

        match self {
            Msg::Vote(vote) => {
                let any = Any {
                    type_url: malachite_proto::Vote::type_url(),
                    value: vote.into_bytes()?,
                };

                Ok(any.encode_to_vec())
            }
            Msg::Proposal(proposal) => {
                let any = Any {
                    type_url: malachite_proto::Proposal::type_url(),
                    value: proposal.into_bytes()?,
                };

                Ok(any.encode_to_vec())
            }

            #[cfg(test)]
            Msg::Dummy(value) => {
                let any = Any {
                    type_url: "malachite.proto.Dummy".to_string(),
                    value: value.to_be_bytes().to_vec(),
                };

                Ok(any.encode_to_vec())
            }
        }
    }
}

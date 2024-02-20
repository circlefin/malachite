use std::convert::Infallible;

use thiserror::Error;

use prost::{DecodeError, EncodeError, Message};

include!(concat!(env!("OUT_DIR"), "/malachite.rs"));

mod impls;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to decode Protobuf message")]
    Decode(#[from] DecodeError),

    #[error("Failed to encode Protobuf message")]
    Encode(#[from] EncodeError),

    #[error("{0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub trait Protobuf
where
    Self: Sized + TryFrom<Self::Proto>,
    Error: From<Self::Error>,
{
    type Proto: Message + Default + From<Self>;

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let proto = Self::Proto::decode(bytes)?;
        let result = Self::try_from(proto)?;
        Ok(result)
    }

    fn into_bytes(self) -> Result<Vec<u8>, Error> {
        let proto = Self::Proto::from(self);
        let mut bytes = Vec::with_capacity(proto.encoded_len());
        proto.encode(&mut bytes)?;
        Ok(bytes)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, Error>
    where
        Self: Clone,
    {
        Protobuf::into_bytes(self.clone())
    }
}

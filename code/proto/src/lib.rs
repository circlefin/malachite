use std::fmt::Display;

use thiserror::Error;

use prost::{DecodeError, EncodeError, Message};

include!(concat!(env!("OUT_DIR"), "/malachite.rs"));

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to decode Protobuf message")]
    Decode(#[from] DecodeError),

    #[error("Failed to encode Protobuf message")]
    Encode(#[from] EncodeError),

    #[error("{0}")]
    Other(String),
}

pub trait Protobuf {
    type Proto: Message + Default;

    fn from_bytes(bytes: &[u8]) -> Result<Self, Error>
    where
        Self: TryFrom<Self::Proto> + Sized, // FIXME: Require `TryFrom<&Self::Proto>` instead
        Self::Error: Display,               // FIXME: Require `std::error::Error` instead
    {
        let proto = Self::Proto::decode(bytes)?;
        Self::try_from(proto).map_err(|e| Error::Other(e.to_string()))
    }

    fn into_bytes(self) -> Result<Vec<u8>, Error>
    where
        Self: Sized,
        Self::Proto: From<Self>,
    {
        let proto = Self::Proto::from(self);
        let mut bytes = Vec::with_capacity(proto.encoded_len());
        proto.encode(&mut bytes)?;
        Ok(bytes)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, Error>
    where
        Self: Sized + Clone,
        Self::Proto: From<Self>,
    {
        Protobuf::into_bytes(self.clone())
    }
}

use bytes::{Bytes, BytesMut};
use core::fmt;
use malachitebft_proto::{Error as ProtoError, Protobuf};
use serde::{Deserialize, Serialize};

use crate::proto;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Serialize, Deserialize)]
pub struct ValueId(u64);

impl ValueId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for ValueId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl Protobuf for ValueId {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let bytes = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        let len = bytes.len();
        let bytes = <[u8; 8]>::try_from(bytes.as_ref()).map_err(|_| {
            ProtoError::Other(format!(
                "Invalid value length, got {len} bytes expected {}",
                u64::BITS / 8
            ))
        })?;

        Ok(ValueId::new(u64::from_be_bytes(bytes)))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::ValueId {
            value: Some(self.0.to_be_bytes().to_vec().into()),
        })
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Value {
    pub value: u64,
    pub extensions: Bytes,
}

impl Value {
    pub fn new(value: u64) -> Self {
        Self {
            value,
            extensions: Bytes::new(),
        }
    }

    pub fn id(&self) -> ValueId {
        ValueId(self.value)
    }

    pub fn size_bytes(&self) -> usize {
        std::mem::size_of_val(&self.value) + self.extensions.len()
    }
}

impl malachitebft_core_types::Value for Value {
    type Id = ValueId;

    fn id(&self) -> ValueId {
        self.id()
    }
}

impl Protobuf for Value {
    type Proto = proto::Value;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let bytes = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        let value = bytes[0..8].try_into().map_err(|_| {
            ProtoError::Other(format!(
                "Too few bytes, expected at least {}",
                u64::BITS / 8
            ))
        })?;

        let extensions = bytes.slice(8..);

        Ok(Value {
            value: u64::from_be_bytes(value),
            extensions,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        let mut bytes = BytesMut::new();
        bytes.extend_from_slice(&self.value.to_be_bytes());
        bytes.extend_from_slice(&self.extensions);

        Ok(proto::Value {
            value: Some(bytes.freeze()),
        })
    }
}

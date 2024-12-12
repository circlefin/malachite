use core::fmt;
use malachite_proto::{Error as ProtoError, Protobuf};
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
        let bytes = <[u8; 8]>::try_from(bytes).map_err(|_| {
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
            value: Some(self.0.to_be_bytes().to_vec()),
        })
    }
}

/// The value to decide on
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Value(u64);

impl Value {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub const fn id(&self) -> ValueId {
        ValueId(self.0)
    }

    pub fn size_bytes(&self) -> usize {
        8
    }
}

impl malachite_core_types::Value for Value {
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

        let len = bytes.len();
        let bytes = <[u8; 8]>::try_from(bytes).map_err(|_| {
            ProtoError::Other(format!(
                "Invalid value length, got {len} bytes expected {}",
                u64::BITS / 8
            ))
        })?;

        Ok(Value::new(u64::from_be_bytes(bytes)))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::Value {
            value: Some(self.0.to_be_bytes().to_vec()),
        })
    }
}

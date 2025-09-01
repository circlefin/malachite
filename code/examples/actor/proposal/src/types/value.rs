use bytes::Bytes;
use core::fmt;
use malachitebft_proto::{Error as ProtoError, Protobuf};
use serde::{Deserialize, Serialize};

use super::block::Block;
use super::hash::Hash;
use super::proto;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValueId(Hash);

impl ValueId {
    pub fn new(id: Hash) -> Self {
        Self(id)
    }

    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<Hash> for ValueId {
    fn from(value: Hash) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Protobuf for ValueId {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let bytes = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        Ok(ValueId::new(Hash::from_bytes(&bytes.elements).map_err(
            |_| ProtoError::invalid_data::<Self::Proto>("invalid hash"),
        )?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::ValueId {
            value: Some(proto::Hash {
                elements: Bytes::from(self.0.to_vec()),
            }),
        })
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Value {
    pub value: Block,
}

impl Value {
    pub fn new(value: Block) -> Self {
        Self { value }
    }

    pub fn id(&self) -> ValueId {
        ValueId::new(self.value.block_hash.clone())
    }

    pub fn size_bytes(&self) -> usize {
        std::mem::size_of_val(&self.value)
    }
}

impl Protobuf for Value {
    type Proto = proto::Value;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let value = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        Ok(Value {
            value: Block::from_proto(value)
                .map_err(|_| ProtoError::invalid_data::<Self::Proto>("invalid block"))?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::Value {
            value: Some(self.value.to_proto()?),
        })
    }
}

impl malachitebft_core_types::Value for Value {
    type Id = ValueId;

    fn id(&self) -> ValueId {
        ValueId::new(self.value.block_hash.clone())
    }
}

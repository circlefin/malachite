use core::fmt;
use serde::{Deserialize, Serialize};

use crate::types::signing::{Hashable, PublicKey};
use malachitebft_proto::{Error as ProtoError, Protobuf};

use super::proto;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(
    #[serde(
        serialize_with = "hex::serde::serialize_upper",
        deserialize_with = "hex::serde::deserialize"
    )]
    [u8; Self::LENGTH],
);

impl Address {
    const LENGTH: usize = 20;

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn new(value: [u8; Self::LENGTH]) -> Self {
        Self(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_public_key(public_key: &PublicKey) -> Self {
        let hash = public_key.hash();
        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&hash[..Self::LENGTH]);
        Self(address)
    }

    pub fn into_inner(self) -> [u8; Self::LENGTH] {
        self.0
    }
}

impl fmt::Display for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({self})")
    }
}

impl malachitebft_core_types::Address for Address {}

impl Protobuf for Address {
    type Proto = proto::Address;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.value.len() != Self::LENGTH {
            return Err(ProtoError::Other(format!(
                "Invalid address length: expected {}, got {}",
                Self::LENGTH,
                proto.value.len()
            )));
        }

        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&proto.value);
        Ok(Self(address))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::Address {
            value: self.0.to_vec().into(),
        })
    }
}

use bytes::Bytes;
use core::fmt;
use serde::{Deserialize, Serialize};

use malachitebft_proto::{Error as ProtoError, Protobuf};
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::PublicKey;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(PublicKey);

impl Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(bytes: [u8; 32]) -> Self {
        Self::from_public_key(PublicKey::from_bytes(bytes))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_public_key(public_key: PublicKey) -> Self {
        Self(public_key)
    }
}

impl fmt::Display for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_bytes().iter() {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", self)
    }
}

impl malachitebft_core_types::Address for Address {}

impl Protobuf for Address {
    type Proto = p2p_proto::Address;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.elements.len() != 32 {
            return Err(ProtoError::Other(format!(
                "Invalid address length: expected 32, got {}",
                proto.elements.len()
            )));
        }

        let mut bytes = [0; 32];
        bytes.copy_from_slice(&proto.elements);
        Ok(Address::new(bytes))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(p2p_proto::Address {
            elements: Bytes::copy_from_slice(self.0.as_bytes().as_slice()),
        })
    }
}

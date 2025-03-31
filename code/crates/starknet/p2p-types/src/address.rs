use bytes::Bytes;
use core::fmt;
use malachitebft_signing_ed25519::PublicKey;
use serde::{Deserialize, Serialize};

use malachitebft_proto::{Error as ProtoError, Protobuf};
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::Felt;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(Felt);

impl Address {
    pub fn new(felt: Felt) -> Self {
        Self(felt)
    }
}

impl From<u64> for Address {
    fn from(n: u64) -> Self {
        Self(Felt::from(n))
    }
}

impl From<[u8; 32]> for Address {
    fn from(bytes: [u8; 32]) -> Self {
        Self(Felt::from_bytes_be(&bytes))
    }
}

impl From<PublicKey> for Address {
    fn from(public_key: PublicKey) -> Self {
        Self::from(*public_key.as_bytes())
    }
}

impl From<&PublicKey> for Address {
    fn from(public_key: &PublicKey) -> Self {
        Self::from(*public_key.as_bytes())
    }
}

impl fmt::Display for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::Debug for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({:#x})", self.0)
    }
}

impl malachitebft_core_types::Address for Address {}

impl Protobuf for Address {
    type Proto = p2p_proto::Address;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let mut felt = [0; 32];
        if proto.elements.len() != 32 {
            return Err(ProtoError::invalid_data::<Self::Proto>("elements"));
        }

        felt.copy_from_slice(&proto.elements);

        let hash = Felt::from_bytes_be(&felt);
        Ok(Self(hash))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(p2p_proto::Address {
            elements: Bytes::from(self.0.to_bytes_be().to_vec()),
        })
    }
}

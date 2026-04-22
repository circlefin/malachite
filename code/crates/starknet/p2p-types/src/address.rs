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

        let public_key = PublicKey::try_from_bytes(bytes).map_err(|e| {
            ProtoError::Other(format!("Invalid public key bytes: {e}"))
        })?;

        Ok(Address::from_public_key(public_key))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(p2p_proto::Address {
            elements: Bytes::copy_from_slice(self.0.as_bytes().as_slice()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_proto_rejects_invalid_public_key_bytes() {
        // Scan for a 32-byte value that ed25519-consensus rejects.
        // Roughly half of all y-coordinates fail point decompression.
        let mut bytes = [0u8; 32];
        for candidate in 0u8..=255 {
            bytes[0] = candidate;
            if PublicKey::try_from_bytes(bytes).is_err() {
                let proto = p2p_proto::Address {
                    elements: Bytes::copy_from_slice(&bytes),
                };

                let result = Address::from_proto(proto);
                assert!(result.is_err(), "expected error for invalid public key bytes");

                let err_msg = result.unwrap_err().to_string();
                assert!(
                    err_msg.contains("Invalid public key bytes"),
                    "unexpected error message: {err_msg}"
                );
                return;
            }
        }
        panic!("could not find invalid Ed25519 public key bytes for test");
    }

    #[test]
    fn from_proto_rejects_wrong_length() {
        let proto = p2p_proto::Address {
            elements: Bytes::from_static(&[0; 16]),
        };

        let result = Address::from_proto(proto);
        assert!(result.is_err());
    }
}

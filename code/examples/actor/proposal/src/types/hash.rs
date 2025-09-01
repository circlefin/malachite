use bytes::Bytes;
use sha3::{Digest, Sha3_256};
use std::fmt;

use serde::{Deserialize, Serialize};

use malachitebft_proto::{Error as ProtoError, Protobuf};

use super::proto;
// Define the size of the hash (32 bytes for SHA3-256).
const HASH_SIZE: usize = 32;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hash([u8; HASH_SIZE]);

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl Hash {
    /// Create a new `Hash` from a fixed-size array of 32 bytes.
    pub fn new(data: [u8; HASH_SIZE]) -> Hash {
        Hash(data)
    }

    /// Create an empty `Hash` initialized to zero.
    pub fn new_empty() -> Hash {
        Hash([0; HASH_SIZE])
    }

    /// Get the hash as a vector of bytes.
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// Get the hash as a byte slice.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert the hash to a hexadecimal string.
    pub fn to_hex_string(&self) -> String {
        self.0
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }

    /// Compare this hash with another for equality.
    pub fn is_eq(&self, other: &Hash) -> bool {
        self.0 == other.0
    }

    /// Create a `Hash` from a slice of bytes.
    /// Returns an error if the slice length is not 32.
    pub fn from_bytes(bytes: &[u8]) -> Result<Hash, &'static str> {
        if bytes.len() != HASH_SIZE {
            return Err("Invalid hash length");
        }
        let mut hash = [0u8; HASH_SIZE];
        hash.copy_from_slice(bytes);
        Ok(Hash(hash))
    }

    /// Compute a SHA3-256 hash of the input data and return it as a `Hash` instance.
    pub fn compute_hash(data: &[u8]) -> Hash {
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        let sha3_256_hash = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&sha3_256_hash);
        Hash(hash)
    }
}

impl Protobuf for Hash {
    type Proto = proto::Hash;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Hash::from_bytes(&proto.elements)
            .map_err(|_| ProtoError::invalid_data::<Self::Proto>("invalid hash"))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(proto::Hash {
            elements: Bytes::from(self.to_vec()),
        })
    }
}

pub type BlockHash = Hash;

use core::{fmt, str};

use serde::{Deserialize, Serialize};

use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;
use starknet_core::types::Hash256;

pub type MessageHash = Hash;
pub type BlockHash = Hash;

impl malachite_common::Value for BlockHash {
    type Id = BlockHash;

    fn id(&self) -> Self::Id {
        *self
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hash(Hash256);

impl Hash {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(Hash256::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }
}

impl PartialOrd for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl proto::Protobuf for Hash {
    type Proto = p2p_proto::Hash;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self::new(proto.elements.try_into().map_err(|_| {
            proto::Error::Other("Invalid hash length".to_string())
        })?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(p2p_proto::Hash {
            elements: self.as_bytes().to_vec(),
        })
    }
}

impl fmt::Display for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl str::FromStr for Hash {
    type Err = Box<dyn std::error::Error>;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hash = str::FromStr::from_str(s)?;
        Ok(Self(hash))
    }
}

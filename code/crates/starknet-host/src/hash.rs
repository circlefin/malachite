use core::fmt;

use subtle_encoding::hex;

use malachite_proto as proto;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[allow(clippy::len_without_is_empty)]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

impl proto::Protobuf for Hash {
    type Proto = crate::proto::mock::Hash;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self::new(proto.hash.try_into().map_err(|_| {
            proto::Error::Other("Invalid hash length".to_string())
        })?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(crate::proto::mock::Hash {
            hash: self.0.to_vec(),
        })
    }
}

impl fmt::Display for Hash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::str::from_utf8(&hex::encode(self.0)).unwrap().fmt(f)
    }
}

impl core::str::FromStr for Hash {
    type Err = Box<dyn std::error::Error>;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

pub type MessageHash = Hash;
pub type BlockHash = Hash;

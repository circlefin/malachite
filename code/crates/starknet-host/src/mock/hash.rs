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
}

impl proto::Protobuf for Hash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self::new(
            proto
                .value
                .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("value"))?
                .try_into()
                .map_err(|_| proto::Error::Other("Invalid hash length".to_string()))?,
        ))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::ValueId {
            value: Some(self.0.to_vec()),
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockHash(Hash);

impl BlockHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(Hash::new(hash))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlockHash({})", self.0)
    }
}

impl core::str::FromStr for BlockHash {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

impl proto::Protobuf for BlockHash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self(Hash::from_proto(proto)?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        self.0.to_proto()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MessageHash(Hash);

impl MessageHash {
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(Hash::new(hash))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Display for MessageHash {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessageHash({})", self.0)
    }
}

impl core::str::FromStr for MessageHash {
    type Err = Box<dyn std::error::Error>;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(hex::decode(s)?.as_slice().try_into()?))
    }
}

impl proto::Protobuf for MessageHash {
    type Proto = proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self(Hash::from_proto(proto)?))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        self.0.to_proto()
    }
}

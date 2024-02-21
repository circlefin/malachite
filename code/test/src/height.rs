use core::fmt;

use malachite_proto as proto;

/// A blockchain height
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(u64);

impl Height {
    pub fn new(height: u64) -> Self {
        Self(height)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl malachite_common::Height for Height {
    fn increment(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl proto::Protobuf for Height {
    type Proto = proto::Height;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self(proto.value))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Height { value: self.0 })
    }
}

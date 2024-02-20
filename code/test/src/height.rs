use std::convert::Infallible;

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

impl malachite_common::Height for Height {}

impl TryFrom<malachite_proto::Height> for Height {
    type Error = Infallible;

    fn try_from(height: malachite_proto::Height) -> Result<Self, Self::Error> {
        Ok(Self(height.value))
    }
}

impl From<Height> for malachite_proto::Height {
    fn from(height: Height) -> malachite_proto::Height {
        malachite_proto::Height { value: height.0 }
    }
}

impl malachite_proto::Protobuf for Height {
    type Proto = malachite_proto::Height;
}

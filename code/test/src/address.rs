use core::fmt;

use malachite_proto as proto;

use crate::signing::PublicKey;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address([u8; Self::LENGTH]);

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
}

impl fmt::Display for Address {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
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

impl malachite_common::Address for Address {}

impl malachite_proto::Protobuf for Address {
    type Proto = proto::Address;

    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        if proto.value.len() != Self::LENGTH {
            return Err(proto::Error::Other(format!(
                "Invalid address length: expected {}, got {}",
                Self::LENGTH,
                proto.value.len()
            )));
        }

        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&proto.value);
        Ok(Self(address))
    }

    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(proto::Address {
            value: self.0.to_vec(),
        })
    }
}

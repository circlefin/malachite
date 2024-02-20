use core::fmt;

use crate::signing::PublicKey;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

impl malachite_common::Address for Address {}

impl TryFrom<malachite_proto::Address> for Address {
    type Error = String;

    fn try_from(proto: malachite_proto::Address) -> Result<Self, Self::Error> {
        if proto.value.len() != Self::LENGTH {
            return Err(format!(
                "Invalid address length: expected {}, got {}",
                Self::LENGTH,
                proto.value.len()
            ));
        }

        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&proto.value);
        Ok(Self(address))
    }
}

impl From<Address> for malachite_proto::Address {
    fn from(address: Address) -> Self {
        Self {
            value: address.0.to_vec(),
        }
    }
}

impl malachite_proto::Protobuf for Address {
    type Proto = malachite_proto::Address;
}

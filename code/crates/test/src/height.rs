use core::fmt;
use malachitebft_proto::{Error as ProtoError, Protobuf};
use serde::{Deserialize, Serialize};

/// A blockchain height
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Height(u64);

impl Height {
    pub const fn new(height: u64) -> Self {
        Self(height)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn decrement(&self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

impl Default for Height {
    fn default() -> Self {
        malachitebft_core_types::Height::ZERO
    }
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Height({})", self.0)
    }
}

impl malachitebft_core_types::Height for Height {
    const ZERO: Self = Self(0);
    const INITIAL: Self = Self(1);

    fn increment_by(&self, n: u64) -> Self {
        Self(self.0 + n)
    }

    fn decrement_by(&self, n: u64) -> Option<Self> {
        self.0.checked_sub(n).map(Self)
    }

    fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Protobuf for Height {
    type Proto = u64;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self(proto))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use malachitebft_core_types::Height as _;

    #[test]
    fn decrement_by_returns_none_on_underflow() {
        // The Height trait documents decrement_by as returning None when the
        // result would go below the minimum, and its default decrement()
        // delegates here. It must agree with the inherent decrement(), which
        // already uses checked_sub, rather than saturating to Some(0).
        assert_eq!(Height::new(5).decrement_by(5).map(|h| h.as_u64()), Some(0));
        assert_eq!(Height::new(5).decrement_by(6), None);
        assert_eq!(
            <Height as malachitebft_core_types::Height>::decrement(&Height::ZERO),
            None,
        );

        // Trait decrement() (via decrement_by) must match inherent decrement().
        let zero = Height::new(0);
        assert_eq!(
            <Height as malachitebft_core_types::Height>::decrement(&zero),
            zero.decrement(),
        );
    }
}

use core::fmt;

/// A blockchain height
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Height {
    pub block_number: u64,
    pub fork_id: u64,
}

impl Height {
    pub const fn new(block_number: u64, fork_id: u64) -> Self {
        Self {
            block_number,
            fork_id,
        }
    }

    pub const fn as_u64(&self) -> u64 {
        self.block_number
    }

    pub const fn increment(&self) -> Self {
        self.increment_by(1)
    }

    pub const fn increment_by(&self, n: u64) -> Self {
        Self {
            block_number: self.block_number + n,
            fork_id: self.fork_id,
        }
    }

    pub fn decrement(&self) -> Option<Self> {
        self.block_number.checked_sub(1).map(|block_number| Self {
            block_number,
            fork_id: self.fork_id,
        })
    }
}

impl fmt::Display for Height {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.block_number.fmt(f)
    }
}

impl malachite_core_types::Height for Height {
    fn increment_by(&self, n: u64) -> Self {
        Self {
            block_number: self.block_number + n,
            fork_id: self.fork_id,
        }
    }

    fn decrement_by(&self, n: u64) -> Option<Self> {
        Some(Self {
            block_number: self.block_number.saturating_sub(n),
            fork_id: self.fork_id,
        })
    }

    fn as_u64(&self) -> u64 {
        self.block_number
    }
}

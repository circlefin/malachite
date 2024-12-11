use core::fmt::{Debug, Display};

/// Defines the requirements for a height type.
///
/// A height denotes the number of blocks (values) created since the chain began.
///
/// A height of 0 represents a chain which has not yet produced a block.
pub trait Height
where
    Self:
        Default + Copy + Clone + Debug + Display + PartialEq + Eq + PartialOrd + Ord + Send + Sync,
{
    /// Increment the height by one.
    fn increment(&self) -> Self {
        self.increment_by(1)
    }

    /// Decrement the height by one.
    fn decrement(&self) -> Option<Self> {
        self.decrement_by(1)
    }

    /// Increment this height by the given amount.
    fn increment_by(&self, n: u64) -> Self;

    /// Decrement this height by the given amount.
    /// Returns None if the height would be decremented below its minimum.
    fn decrement_by(&self, n: u64) -> Option<Self>;

    /// Convert the height to a `u64`.
    fn as_u64(&self) -> u64;
}

use core::fmt::{Debug, Display};

/// Defines the requirements for a height type.
///
/// A height denotes the number of blocks (values) created since the chain began.
///
/// A height of 0 represents a chain which has not yet produced a block.
pub trait Height
where
    Self: Copy + Clone + Default + Debug + Display + Eq + Ord + Send + Sync,
{
    /// The zero-th height. Typically 0.
    ///
    /// This value must be the same as the one built by the `Default` impl.
    const ZERO: Self;

    /// The initial height. Typically 1.
    const INITIAL: Self;

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

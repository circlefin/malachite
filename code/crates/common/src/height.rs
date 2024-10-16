use core::fmt::{self, Debug, Display};
use core::ops::RangeInclusive;

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

    /// Increment this height by the given amount.
    fn increment_by(&self, n: u64) -> Self;

    /// Convert the height to a `u64`.
    fn as_u64(&self) -> u64;
}

/// A range of heights, starting from `start` and ending at `end`, inclusive.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InclusiveRange<H> {
    /// Start of the range, inclusive
    pub start: H,
    /// End of the range, inclusive
    pub end: H,
}

impl<H> InclusiveRange<H> {
    /// Create a new inclusive range from `start` to `end`.
    pub fn new(start: H, end: H) -> Self {
        Self { start, end }
    }

    /// Get the length of the range.
    pub fn len(&self) -> usize
    where
        H: Height,
    {
        (self.end.as_u64() - self.start.as_u64() + 1) as _
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool
    where
        H: Height,
    {
        self.len() == 0
    }
}

impl<H> From<RangeInclusive<H>> for InclusiveRange<H>
where
    H: Height,
{
    fn from(range: RangeInclusive<H>) -> Self {
        Self::new(*range.start(), *range.end())
    }
}

impl<H> Iterator for InclusiveRange<H>
where
    H: Height,
{
    type Item = H;

    fn next(&mut self) -> Option<H> {
        if self.start <= self.end {
            let result = self.start;
            self.start = self.start.increment();
            Some(result)
        } else {
            None
        }
    }
}

impl<H> fmt::Display for InclusiveRange<H>
where
    H: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..={}", self.start, self.end)
    }
}

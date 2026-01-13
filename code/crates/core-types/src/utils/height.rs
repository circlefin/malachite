//! Utilities for working with heights.

use core::ops::RangeInclusive;

use crate::Height;

/// A helper struct for displaying ranges of heights.
pub struct DisplayRange<'a, H>(pub &'a RangeInclusive<H>);

impl<'a, H: Height> core::fmt::Display for DisplayRange<'a, H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..={}", self.0.start(), self.0.end())
    }
}

/// Extension trait for iterating over ranges of heights.
pub trait HeightRangeExt {
    /// The iterator type.
    type Iter;

    /// Returns an iterator over the heights in the range.
    fn iter_heights(self) -> Self::Iter;

    /// Returns the length of the range.
    fn len(&self) -> usize;

    /// Returns true if the range is empty.
    fn is_empty(&self) -> bool;
}

impl<T> HeightRangeExt for RangeInclusive<T>
where
    T: Height,
{
    type Iter = HeightRangeInclusiveIterator<T>;

    fn iter_heights(self) -> Self::Iter {
        HeightRangeInclusiveIterator {
            current: *self.start(),
            end: *self.end(),
        }
    }

    fn len(&self) -> usize {
        let start = self.start().as_u64();
        let end = self.end().as_u64();

        if end < start {
            0
        } else {
            (end - start).saturating_add(1) as usize
        }
    }

    fn is_empty(&self) -> bool {
        self.end() < self.start()
    }
}

impl<H> From<RangeInclusive<H>> for HeightRangeInclusiveIterator<H>
where
    H: Height,
{
    fn from(range: RangeInclusive<H>) -> Self {
        HeightRangeInclusiveIterator {
            current: *range.start(),
            end: *range.end(),
        }
    }
}

/// An iterator over a range of heights.
pub struct HeightRangeInclusiveIterator<H>
where
    H: Height,
{
    current: H,
    end: H,
}

impl<H> Iterator for HeightRangeInclusiveIterator<H>
where
    H: Height,
{
    type Item = H;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.end {
            None
        } else {
            let next = self.current;
            self.current = self.current.increment();
            Some(next)
        }
    }
}

impl<H> ExactSizeIterator for HeightRangeInclusiveIterator<H>
where
    H: Height,
{
    fn len(&self) -> usize {
        let start = self.current.as_u64();
        let end = self.end.as_u64();

        if end < start {
            0
        } else {
            (end - start).saturating_add(1) as usize
        }
    }
}

impl<H> DoubleEndedIterator for HeightRangeInclusiveIterator<H>
where
    H: Height,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current > self.end {
            None
        } else {
            let next = self.end;
            self.end = self.end.decrement().unwrap_or(H::ZERO);
            Some(next)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Default, Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct TestHeight(u64);

    impl core::fmt::Display for TestHeight {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl Height for TestHeight {
        const ZERO: Self = TestHeight(0);
        const INITIAL: Self = TestHeight(1);

        fn increment_by(&self, n: u64) -> Self {
            TestHeight(self.0 + n)
        }

        fn decrement_by(&self, n: u64) -> Option<Self> {
            if self.0 >= n {
                Some(TestHeight(self.0 - n))
            } else {
                None
            }
        }

        fn as_u64(&self) -> u64 {
            self.0
        }
    }

    #[test]
    fn range_inclusive_iterator() {
        let start = TestHeight(3);
        let end = TestHeight(6);
        let range = start..=end;
        let mut iter = range.iter_heights();

        assert_eq!(iter.next(), Some(TestHeight(3)));
        assert_eq!(iter.next(), Some(TestHeight(4)));
        assert_eq!(iter.next(), Some(TestHeight(5)));
        assert_eq!(iter.next(), Some(TestHeight(6)));
        assert_eq!(iter.next(), None);

        let empty_range = TestHeight(5)..=TestHeight(4);
        let mut empty_iter = empty_range.iter_heights();
        assert_eq!(empty_iter.next(), None);
    }

    #[test]
    fn exact_size_iterator_len() {
        let range = TestHeight(2)..=TestHeight(5);
        let iter = range.iter_heights();
        assert_eq!(iter.len(), 4);

        let empty_range = TestHeight(5)..=TestHeight(4);
        let empty_iter = empty_range.iter_heights();
        assert_eq!(empty_iter.len(), 0);

        let max_range = TestHeight(0)..=TestHeight(u64::MAX);
        let max_iter = max_range.iter_heights();
        assert_eq!(max_iter.len(), (u64::MAX as usize));
    }

    #[test]
    fn double_ended_iterator() {
        let range = TestHeight(1)..=TestHeight(4);
        let mut iter = range.iter_heights();

        assert_eq!(iter.next_back(), Some(TestHeight(4)));
        assert_eq!(iter.next_back(), Some(TestHeight(3)));
        assert_eq!(iter.next(), Some(TestHeight(1)));
        assert_eq!(iter.next(), Some(TestHeight(2)));
        assert_eq!(iter.next_back(), None);
        assert_eq!(iter.next(), None);
    }
}

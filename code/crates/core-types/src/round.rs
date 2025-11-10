use core::{cmp, fmt};

/// A round number.
///
/// Can be either:
/// - `Round::Nil` (ie. `-1`)
/// - `Round::Some(r)` where `r >= 0`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "borsh",
    derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize)
)]
pub enum Round {
    /// No round, ie. `-1`
    Nil,

    /// Some round `r` where `r >= 0`
    Some(u32),
}

impl Round {
    /// The zero-th or initial round.
    pub const ZERO: Self = Self::Some(0);

    /// Create a new non-nil round.
    pub const fn new(round: u32) -> Self {
        Self::Some(round)
    }

    /// Convert a round to a `Option<u32>`.
    ///
    /// `Round::Nil` is converted to `None`.
    /// `Round::Some(r)` is converted to `Some(r)`.
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Round::Nil => None,
            Round::Some(r) => Some(*r),
        }
    }

    /// Convert the round to an `i64`.
    ///
    /// `Round::Nil` is converted to `-1`.
    /// `Round::Some(r)` is converted to `r`.
    pub fn as_i64(&self) -> i64 {
        match self {
            Round::Nil => -1,
            Round::Some(r) => i64::from(*r),
        }
    }

    /// Whether the round is defined, ie. `r >= 0`.
    pub fn is_defined(&self) -> bool {
        matches!(self, Round::Some(_))
    }

    /// Whether the round is nil, ie. `r == -1`.
    pub fn is_nil(&self) -> bool {
        matches!(self, Round::Nil)
    }

    /// Increment the round.
    ///
    /// If the round is nil, then the initial zero round is returned.
    /// Otherwise, the round is incremented by one.
    pub fn increment(&self) -> Round {
        match self {
            Round::Nil => Round::new(0),
            Round::Some(r) => Round::new(r + 1),
        }
    }

    /// Return `self` if it is defined, otherwise return `round`.
    ///
    /// ```rust
    /// use informalsystems_malachitebft_core_types::Round;
    ///
    /// let nil = Round::Nil;
    /// let some = Round::Some(2);
    ///
    /// assert_eq!(nil.or(Round::Some(5)), Round::Some(5));
    /// assert_eq!(some.or(Round::Some(5)), Round::Some(2));
    ///
    /// assert_eq!(nil.or(Round::Nil), Round::Nil);
    /// assert_eq!(some.or(Round::Nil), Round::Some(2));
    ///
    /// assert_eq!(nil.or(Round::new(10)), Round::new(10));
    /// assert_eq!(some.or(Round::new(10)), Round::Some(2));
    /// ```
    pub fn or(&self, round: Round) -> Round {
        match self {
            Round::Nil => round,
            Round::Some(_) => *self,
        }
    }

    /// Return `self` if it is defined, otherwise compute and return the result of `f`.
    ///
    /// ```rust
    /// use informalsystems_malachitebft_core_types::Round;
    ///
    /// let nil = Round::Nil;
    /// let some = Round::Some(2);
    ///
    /// assert_eq!(nil.or_else(|| Round::Some(5)), Round::Some(5));
    /// assert_eq!(some.or_else(|| Round::Some(5)), Round::Some(2));
    ///
    /// assert_eq!(nil.or_else(|| Round::Nil), Round::Nil);
    /// assert_eq!(some.or_else(|| Round::Nil), Round::Some(2));
    ///
    /// assert_eq!(nil.or_else(|| Round::new(10)), Round::new(10));
    /// assert_eq!(some.or_else(|| Round::new(10)), Round::Some(2));
    /// ```
    pub fn or_else(&self, f: impl FnOnce() -> Round) -> Round {
        match self {
            Round::Nil => f(),
            Round::Some(_) => *self,
        }
    }
}

impl From<u32> for Round {
    fn from(round: u32) -> Self {
        Round::new(round)
    }
}

impl From<Option<u32>> for Round {
    fn from(round: Option<u32>) -> Self {
        match round {
            None => Round::Nil,
            Some(r) => Round::new(r),
        }
    }
}

impl From<i64> for Round {
    fn from(round: i64) -> Self {
        assert!(round <= i64::from(u32::MAX));

        if round < 0 {
            Round::Nil
        } else {
            Round::new(round as u32)
        }
    }
}

impl PartialOrd for Round {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Round {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_i64().cmp(&other.as_i64())
    }
}

impl fmt::Display for Round {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_i64().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        // Test Round::new()
        assert_eq!(Round::new(0), Round::Some(0));
        assert_eq!(Round::new(1), Round::Some(1));
        assert_eq!(Round::new(2), Round::Some(2));

        // Test Round::as_u32()
        assert_eq!(Round::Nil.as_u32(), None);
        assert_eq!(Round::Some(0).as_u32(), Some(0));
        assert_eq!(Round::Some(1).as_u32(), Some(1));
        assert_eq!(Round::Some(2).as_u32(), Some(2));

        // Test Round::as_i64()
        assert_eq!(Round::Nil.as_i64(), -1);
        assert_eq!(Round::Some(0).as_i64(), 0);
        assert_eq!(Round::Some(1).as_i64(), 1);
        assert_eq!(Round::Some(2).as_i64(), 2);

        // Test Round::is_defined()
        assert!(!Round::Nil.is_defined());
        assert!(Round::Some(0).is_defined());
        assert!(Round::Some(1).is_defined());
        assert!(Round::Some(2).is_defined());
    }
}

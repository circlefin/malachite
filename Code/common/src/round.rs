use core::cmp;

/// A round number.
///
/// Can be either:
/// - `Round::Nil` (ie. `-1`)
/// - `Round::Some(r)` where `r >= 0`
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Round {
    /// No round, ie. `-1`
    Nil,

    /// Some round `r` where `r >= 0`
    Some(i64),
}

impl Round {
    /// The initial, zero round.
    pub const INITIAL: Round = Round::new(0);

    /// Create a new round.
    ///
    /// If `round < 0`, then `Round::Nil` is returned.
    /// Otherwise, `Round::Some(round)` is returned.
    pub const fn new(round: i64) -> Self {
        if round < 0 {
            Self::Nil
        } else {
            Self::Some(round)
        }
    }

    /// Convert the round to an `i64`.
    ///
    /// `Round::Nil` is converted to `-1`.
    /// `Round::Some(r)` is converted to `r`.
    pub fn as_i64(&self) -> i64 {
        match self {
            Round::Nil => -1,
            Round::Some(r) => *r,
        }
    }

    /// Wether the round is defined, ie. `Round::Some(r)` where `r >= 0`.
    pub fn is_defined(&self) -> bool {
        matches!(self, Round::Some(r) if *r >= 0)
    }

    /// Wether the round is `Round::Nil`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        // Test Round::new()
        assert_eq!(Round::new(-42), Round::Nil);
        assert_eq!(Round::new(-1), Round::Nil);
        assert_eq!(Round::new(0), Round::Some(0));
        assert_eq!(Round::new(1), Round::Some(1));
        assert_eq!(Round::new(2), Round::Some(2));

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

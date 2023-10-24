/// A round number, ie. a natural number
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Round {
    /// No round
    None,

    /// Some round
    Some(i64),
}

impl Round {
    pub const INITIAL: Round = Round::new(0);

    pub const fn new(round: i64) -> Self {
        if round < 0 {
            Self::None
        } else {
            Self::Some(round)
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Round::None => -1,
            Round::Some(r) => *r,
        }
    }

    pub fn is_defined(&self) -> bool {
        matches!(self, Round::Some(r) if *r >= 0)
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Round::None)
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Round::None => true,
            Round::Some(r) => *r >= 0,
        }
    }

    pub fn increment(&self) -> Round {
        match self {
            Round::None => Round::new(0),
            Round::Some(r) => Round::new(r + 1),
        }
    }
}

impl PartialOrd for Round {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Round {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_i64().cmp(&other.as_i64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        // Test Round::new()
        assert_eq!(Round::new(-42), Round::None);
        assert_eq!(Round::new(-1), Round::None);
        assert_eq!(Round::new(0), Round::Some(0));
        assert_eq!(Round::new(1), Round::Some(1));
        assert_eq!(Round::new(2), Round::Some(2));

        // Test Round::as_i64()
        assert_eq!(Round::None.as_i64(), -1);
        assert_eq!(Round::Some(0).as_i64(), 0);
        assert_eq!(Round::Some(1).as_i64(), 1);
        assert_eq!(Round::Some(2).as_i64(), 2);

        // Test Round::is_defined()
        assert!(!Round::None.is_defined());
        assert!(Round::Some(0).is_defined());
        assert!(Round::Some(1).is_defined());
        assert!(Round::Some(2).is_defined());
    }
}

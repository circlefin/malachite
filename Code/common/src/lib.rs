/// A blockchain height
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(u64);

impl Height {
    pub fn new(height: u64) -> Self {
        Self(height)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// A round number, ie. a natural number
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Round {
    /// No round
    None,

    /// Some round
    Some(i64),
}

impl Round {
    pub fn new(round: i64) -> Self {
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
        matches!(self, Round::Some(_))
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
        self.as_i64().partial_cmp(&other.as_i64())
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value(u64);

impl Value {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub round: Round,
    pub value: Value,
    pub polka_round: Round,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub typ: VoteType,
    pub round: Round,
    pub value: Option<Value>,
}

impl Vote {
    pub fn new_prevote(round: Round, value: Option<Value>) -> Self {
        Self {
            typ: VoteType::Prevote,
            round,
            value,
        }
    }

    pub fn new_precommit(round: Round, value: Option<Value>) -> Self {
        Self {
            typ: VoteType::Precommit,
            round,
            value,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimeoutStep {
    Propose,
    Prevote,
    Precommit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timeout {
    pub round: Round,
    pub step: TimeoutStep,
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

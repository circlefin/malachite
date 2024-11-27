use core::fmt;

use crate::Round;

/// The round step for which the timeout is for.
/// TODO - change to TimeoutType?
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TimeoutStep {
    /// Timeout for the propose step.
    Propose,

    /// Timeout for the prevote step.
    Prevote,

    /// Timeout for detecting consensus being in the prevote step for too long.
    PrevoteTimeLimit,

    /// Timeout for the precommit step.
    Precommit,

    /// Timeout for detecting consensus being in the precommit step for too long.
    PrecommitTimeLimit,

    /// Timeout for the commit step.
    Commit,
}

/// A timeout for a round step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Timeout {
    /// The round for which the timeout is for.
    pub round: Round,

    /// The round step for which the timeout is for.
    /// TODO - change to identifier or name?
    pub step: TimeoutStep,
}

impl Timeout {
    /// Create a new timeout for the given round and step.
    pub const fn new(round: Round, step: TimeoutStep) -> Self {
        Self { round, step }
    }

    /// Create a new timeout for the propose step of the given round.
    pub const fn propose(round: Round) -> Self {
        Self::new(round, TimeoutStep::Propose)
    }

    /// Create a new timeout for the prevote step of the given round.
    pub const fn prevote(round: Round) -> Self {
        Self::new(round, TimeoutStep::Prevote)
    }

    /// Create a new timeout for the prevote step of the given round.
    pub const fn prevote_time_limit(round: Round) -> Self {
        Self::new(round, TimeoutStep::PrevoteTimeLimit)
    }

    /// Create a new timeout for the precommit step of the given round.
    pub const fn precommit(round: Round) -> Self {
        Self::new(round, TimeoutStep::Precommit)
    }
    /// Create a new timeout for the precommit step of the given round.
    pub const fn precommit_time_limit(round: Round) -> Self {
        Self::new(round, TimeoutStep::PrecommitTimeLimit)
    }

    /// Create a new timeout for the commit step of the given round.
    pub const fn commit(round: Round) -> Self {
        Self::new(round, TimeoutStep::Commit)
    }
}

impl fmt::Display for Timeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}Timeout({})", self.step, self.round)
    }
}

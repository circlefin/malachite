use core::fmt;

use crate::Round;

/// The timeout type. There may be multiple timeouts running in a given step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TimeoutKind {
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

    /// Timeout to rebroadcast the last prevote
    PrevoteRebroadcast,

    /// Timeout to rebroadcast the last precommit
    PrecommitRebroadcast,
}

/// A timeout for a round step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Timeout {
    /// The timeout kind.
    pub kind: TimeoutKind,

    /// The round for which the timeout is for.
    pub round: Round,
}

impl Timeout {
    /// Create a new timeout for the given round and step.
    pub const fn new(round: Round, kind: TimeoutKind) -> Self {
        Self { round, kind }
    }

    /// Create a new timeout for the propose step of the given round.
    pub const fn propose(round: Round) -> Self {
        Self::new(round, TimeoutKind::Propose)
    }

    /// Create a new timeout for the prevote step of the given round.
    pub const fn prevote(round: Round) -> Self {
        Self::new(round, TimeoutKind::Prevote)
    }

    /// Create a new timeout for the prevote step of the given round.
    pub const fn prevote_time_limit(round: Round) -> Self {
        Self::new(round, TimeoutKind::PrevoteTimeLimit)
    }

    /// Create a new timeout for the precommit step of the given round.
    pub const fn precommit(round: Round) -> Self {
        Self::new(round, TimeoutKind::Precommit)
    }
    /// Create a new timeout for the precommit step of the given round.
    pub const fn precommit_time_limit(round: Round) -> Self {
        Self::new(round, TimeoutKind::PrecommitTimeLimit)
    }

    /// Create a new timeout for rebroadcasting the last prevote.
    pub const fn prevote_rebroadcast(round: Round) -> Self {
        Self::new(round, TimeoutKind::PrevoteRebroadcast)
    }

    /// Create a new timeout for rebroadcasting the last precommit.
    pub const fn precommit_rebroadcast(round: Round) -> Self {
        Self::new(round, TimeoutKind::PrecommitRebroadcast)
    }
}

impl fmt::Display for Timeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}Timeout({})", self.kind, self.round)
    }
}

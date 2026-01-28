use core::fmt;

use crate::Round;

/// The timeout type. There may be multiple timeouts running in a given step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TimeoutKind {
    /// Timeout for the propose step.
    Propose,

    /// Timeout for the prevote step.
    Prevote,

    /// Timeout for the precommit step.
    Precommit,

    /// Timeout to rebroadcast the round synchronization messages
    Rebroadcast,
}

/// A timeout for a round step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Create a new timeout for the precommit step of the given round.
    pub const fn precommit(round: Round) -> Self {
        Self::new(round, TimeoutKind::Precommit)
    }

    /// Create a new timeout for rebroadcasting the round synchronization messages.
    pub const fn rebroadcast(round: Round) -> Self {
        Self::new(round, TimeoutKind::Rebroadcast)
    }
}

impl fmt::Display for Timeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}Timeout({})", self.kind, self.round)
    }
}

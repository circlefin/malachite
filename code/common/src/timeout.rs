use crate::Round;

/// The round step for which the timeout is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimeoutStep {
    /// The timeout is for the propose step.
    Propose,

    /// The timeout is for the prevote step.
    Prevote,

    /// The timeout is for the precommit step.
    Precommit,
}

/// A timeout for a round step.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timeout {
    /// The round for which the timeout is for.
    pub round: Round,

    /// The round step for which the timeout is for.
    pub step: TimeoutStep,
}

impl Timeout {
    /// Create a new timeout for the given round and step.
    pub fn new(round: Round, step: TimeoutStep) -> Self {
        Self { round, step }
    }

    /// Create a new timeout for the propose step of the given round.
    pub fn propose(round: Round) -> Self {
        Self::new(round, TimeoutStep::Propose)
    }

    /// Create a new timeout for the prevote step of the given round.
    pub fn prevote(round: Round) -> Self {
        Self::new(round, TimeoutStep::Prevote)
    }

    /// Create a new timeout for the precommit step of the given round.
    pub fn precommit(round: Round) -> Self {
        Self::new(round, TimeoutStep::Precommit)
    }
}

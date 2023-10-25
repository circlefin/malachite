use crate::Round;

/// The round step for which the timeout is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimeoutStep {
    Propose,
    Prevote,
    Precommit,
}

/// A timeout for a round step.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timeout {
    pub round: Round,
    pub step: TimeoutStep,
}

impl Timeout {
    pub fn new(round: Round, step: TimeoutStep) -> Self {
        Self { round, step }
    }

    pub fn propose(round: Round) -> Self {
        Self::new(round, TimeoutStep::Propose)
    }

    pub fn prevote(round: Round) -> Self {
        Self::new(round, TimeoutStep::Prevote)
    }

    pub fn precommit(round: Round) -> Self {
        Self::new(round, TimeoutStep::Precommit)
    }
}

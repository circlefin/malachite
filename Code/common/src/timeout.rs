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

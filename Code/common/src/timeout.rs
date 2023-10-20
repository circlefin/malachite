use crate::Round;

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

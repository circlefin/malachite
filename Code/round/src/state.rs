use crate::events::Event;
use crate::state_machine::Transition;
use crate::{Height, Round, Value};
use malachite_common::Proposal;

/// A value and its associated round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundValue {
    pub value: Value,
    pub round: Round,
}

impl RoundValue {
    pub fn new(value: Value, round: Round) -> Self {
        Self { value, round }
    }
}

/// The step of consensus in this round
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    NewRound,
    Propose,
    Prevote,
    Precommit,
    Commit,
}

/// The state of the consensus state machine
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub height: Height,
    pub round: Round,
    pub step: Step,
    pub proposal: Option<Proposal>,
    pub locked: Option<RoundValue>,
    pub valid: Option<RoundValue>,
}

impl State {
    pub fn new(height: Height) -> Self {
        Self {
            height,
            round: Round::INITIAL,
            step: Step::NewRound,
            proposal: None,
            locked: None,
            valid: None,
        }
    }

    pub fn new_round(self, round: Round) -> Self {
        Self {
            round,
            step: Step::NewRound,
            ..self
        }
    }

    pub fn next_step(self) -> Self {
        let step = match self.step {
            Step::NewRound => Step::Propose,
            Step::Propose => Step::Prevote,
            Step::Prevote => Step::Precommit,
            _ => self.step,
        };

        Self { step, ..self }
    }

    pub fn commit_step(self) -> Self {
        Self {
            step: Step::Commit,
            ..self
        }
    }

    pub fn set_locked(self, value: Value) -> Self {
        Self {
            locked: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn set_valid(self, value: Value) -> Self {
        Self {
            valid: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn apply_event(self, round: Round, event: Event) -> Transition {
        crate::state_machine::apply_event(self, round, event)
    }
}

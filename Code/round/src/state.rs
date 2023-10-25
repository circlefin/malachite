use crate::events::Event;
use crate::state_machine::Transition;

use malachite_common::{Consensus, Round};

/// A value and its associated round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundValue<Value> {
    pub value: Value,
    pub round: Round,
}

impl<Value> RoundValue<Value> {
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
#[derive(Debug, PartialEq, Eq)]
pub struct State<C: Consensus> {
    pub height: C::Height,
    pub round: Round,
    pub step: Step,
    pub proposal: Option<C::Proposal>,
    pub locked: Option<RoundValue<C::Value>>,
    pub valid: Option<RoundValue<C::Value>>,
}

impl<C> Clone for State<C>
where
    C: Consensus,
{
    fn clone(&self) -> Self {
        Self {
            height: self.height.clone(),
            round: self.round,
            step: self.step,
            proposal: self.proposal.clone(),
            locked: self.locked.clone(),
            valid: self.valid.clone(),
        }
    }
}

impl<C> State<C>
where
    C: Consensus,
{
    pub fn new(height: C::Height) -> Self {
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

    pub fn set_locked(self, value: C::Value) -> Self {
        Self {
            locked: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn set_valid(self, value: C::Value) -> Self {
        Self {
            valid: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn apply_event(self, round: Round, event: Event<C>) -> Transition<C> {
        crate::state_machine::apply_event(self, round, event)
    }
}

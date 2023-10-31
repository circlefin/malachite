use crate::events::Event;
use crate::state_machine::RoundData;
use crate::transition::Transition;

use malachite_common::{Context, Round};

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
pub struct State<Ctx>
where
    Ctx: Context,
{
    pub round: Round,
    pub step: Step,
    pub proposal: Option<Ctx::Proposal>,
    pub locked: Option<RoundValue<Ctx::Value>>,
    pub valid: Option<RoundValue<Ctx::Value>>,
}

impl<Ctx> Clone for State<Ctx>
where
    Ctx: Context,
{
    fn clone(&self) -> Self {
        Self {
            round: self.round,
            step: self.step,
            proposal: self.proposal.clone(),
            locked: self.locked.clone(),
            valid: self.valid.clone(),
        }
    }
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new() -> Self {
        Self {
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

    pub fn set_locked(self, value: Ctx::Value) -> Self {
        Self {
            locked: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn set_valid(self, value: Ctx::Value) -> Self {
        Self {
            valid: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    pub fn apply_event(self, data: &RoundData<Ctx>, event: Event<Ctx>) -> Transition<Ctx> {
        crate::state_machine::apply_event(self, data, event)
    }
}

impl<Ctx> Default for State<Ctx>
where
    Ctx: Context,
{
    fn default() -> Self {
        Self::new()
    }
}

use core::fmt;

use crate::input::Input;
use crate::state_machine::Info;
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
pub struct State<Ctx>
where
    Ctx: Context,
{
    pub height: Ctx::Height,
    pub round: Round,

    pub step: Step,
    pub proposal: Option<Ctx::Proposal>,
    pub locked: Option<RoundValue<Ctx::Value>>,
    pub valid: Option<RoundValue<Ctx::Value>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(height: Ctx::Height, round: Round) -> Self {
        Self {
            height,
            round,
            step: Step::NewRound,
            proposal: None,
            locked: None,
            valid: None,
        }
    }

    pub fn with_step(self, step: Step) -> Self {
        Self { step, ..self }
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

    pub fn apply(self, data: &Info<Ctx>, input: Input<Ctx>) -> Transition<Ctx> {
        crate::state_machine::apply(self, data, input)
    }
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx> Default for State<Ctx>
where
    Ctx: Context,
{
    fn default() -> Self {
        Self::new(Ctx::Height::default(), Round::Nil)
    }
}

impl<Ctx> Clone for State<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
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

impl<Ctx> fmt::Debug for State<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("height", &self.round)
            .field("round", &self.round)
            .field("step", &self.step)
            .field("proposal", &self.proposal)
            .field("locked", &self.locked)
            .field("valid", &self.valid)
            .finish()
    }
}

impl<Ctx> PartialEq for State<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        self.height == other.height
            && self.round == other.round
            && self.step == other.step
            && self.proposal == other.proposal
            && self.locked == other.locked
            && self.valid == other.valid
    }
}

impl<Ctx> Eq for State<Ctx> where Ctx: Context {}

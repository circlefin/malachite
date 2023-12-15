//! The state maintained by the round state machine

use core::fmt;

use crate::input::Input;
use crate::state_machine::Info;
use crate::transition::Transition;

use malachite_common::{Context, Round};

/// A value and its associated round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundValue<Value> {
    /// The value
    pub value: Value,
    /// The round
    pub round: Round,
}

impl<Value> RoundValue<Value> {
    /// Create a new `RoundValue` instance.
    pub fn new(value: Value, round: Round) -> Self {
        Self { value, round }
    }
}

/// The step of consensus in this round
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// The round has just started
    NewRound,

    /// We are at the propose step.
    /// Either we are the proposer or we are waiting for a proposal.
    Propose,

    /// We are at the prevote step.
    Prevote,

    /// We are at the precommit step.
    Precommit,

    /// We have committed and decided on a value
    Commit,
}

/// The state of the consensus state machine
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// The height of the consensus
    pub height: Ctx::Height,

    /// The round we are at within a height
    pub round: Round,

    /// The step we are at within a round
    pub step: Step,

    /// The value we are locked on, ie. we have received a polka for before we precommitted
    pub locked: Option<RoundValue<Ctx::Value>>,

    /// The value for which we received a polka for after we already precommitted
    pub valid: Option<RoundValue<Ctx::Value>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    /// Create a new `State` instance at the given height and round.
    pub fn new(height: Ctx::Height, round: Round) -> Self {
        Self {
            height,
            round,
            step: Step::NewRound,
            locked: None,
            valid: None,
        }
    }

    /// Set the round.
    pub fn with_round(self, round: Round) -> Self {
        Self { round, ..self }
    }

    /// Set the step.
    pub fn with_step(self, step: Step) -> Self {
        Self { step, ..self }
    }

    /// Set the locked value.
    pub fn set_locked(self, value: Ctx::Value) -> Self {
        Self {
            locked: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    /// Set the valid value.
    pub fn set_valid(self, value: Ctx::Value) -> Self {
        Self {
            valid: Some(RoundValue::new(value, self.round)),
            ..self
        }
    }

    /// Apply the given input to the current state, triggering a transition.
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
            .field("height", &self.height)
            .field("round", &self.round)
            .field("step", &self.step)
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
            && self.locked == other.locked
            && self.valid == other.valid
    }
}

impl<Ctx> Eq for State<Ctx> where Ctx: Context {}

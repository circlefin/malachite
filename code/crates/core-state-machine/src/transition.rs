//! A transition taken by the state machine after processing an input.

use malachite_core_types::Context;

use crate::output::Output;
use crate::state::State;

/// A transition taken by the state machine after processing an input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition<Ctx>
where
    Ctx: Context,
{
    /// The next state to transition to.
    pub next_state: State<Ctx>,
    /// The output to emit.
    pub output: Option<Output<Ctx>>,
    /// Whether the transition is valid or not.
    pub valid: bool,
}

impl<Ctx> Transition<Ctx>
where
    Ctx: Context,
{
    /// Build a new valid transition to the given next state.
    pub fn to(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            output: None,
            valid: true,
        }
    }

    /// Build a new invalid transition to the given next state.
    pub fn invalid(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            output: None,
            valid: false,
        }
    }

    /// Set the output of the transition.
    pub fn with_output(mut self, output: Output<Ctx>) -> Self {
        self.output = Some(output);
        self
    }
}

use malachite_common::Context;

use crate::output::Output;
use crate::state::State;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition<Ctx>
where
    Ctx: Context,
{
    pub next_state: State<Ctx>,
    pub output: Option<Output<Ctx>>,
    pub valid: bool,
}

impl<Ctx> Transition<Ctx>
where
    Ctx: Context,
{
    pub fn to(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            output: None,
            valid: true,
        }
    }

    pub fn invalid(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            output: None,
            valid: false,
        }
    }

    pub fn with_output(mut self, output: Output<Ctx>) -> Self {
        self.output = Some(output);
        self
    }
}

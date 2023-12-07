use core::fmt;

use malachite_common::Context;

use crate::output::Output;
use crate::state::State;

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

impl<Ctx: Context> Clone for Transition<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        Self {
            next_state: self.next_state.clone(),
            output: self.output.clone(),
            valid: self.valid,
        }
    }
}

impl<Ctx: Context> fmt::Debug for Transition<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transition")
            .field("next_state", &self.next_state)
            .field("output", &self.output)
            .field("valid", &self.valid)
            .finish()
    }
}

impl<Ctx: Context> PartialEq for Transition<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        self.next_state == other.next_state
            && self.output == other.output
            && self.valid == other.valid
    }
}

impl<Ctx: Context> Eq for Transition<Ctx> {}

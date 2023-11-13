use malachite_common::Context;

use crate::message::Message;
use crate::state::State;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition<Ctx>
where
    Ctx: Context,
{
    pub next_state: State<Ctx>,
    pub message: Option<Message<Ctx>>,
    pub valid: bool,
}

impl<Ctx> Transition<Ctx>
where
    Ctx: Context,
{
    pub fn to(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            message: None,
            valid: true,
        }
    }

    pub fn invalid(next_state: State<Ctx>) -> Self {
        Self {
            next_state,
            message: None,
            valid: false,
        }
    }

    pub fn with_message(mut self, message: Message<Ctx>) -> Self {
        self.message = Some(message);
        self
    }
}

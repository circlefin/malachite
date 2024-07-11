use std::future::Future;
use std::pin::Pin;

use corosensei::stack::DefaultStack;
use corosensei::ScopedCoroutine;

use malachite_common::*;
use malachite_metrics::Metrics;

use crate::effect::{Effect, Resume};
use crate::error::Error;
use crate::handle::handle_msg;
use crate::msg::Msg;
use crate::state::State;

type Co<'a, Ctx> =
    ScopedCoroutine<'a, Resume<Ctx>, Effect<Ctx>, Result<(), Error<Ctx>>, DefaultStack>;

type CoResult<Ctx> = corosensei::CoroutineResult<Effect<Ctx>, Result<(), Error<Ctx>>>;

/// Process a message synchronously.
///
/// # Example
/// TODO
pub fn process_sync<'a, Ctx>(
    state: &'a mut State<Ctx>,
    metrics: &'a Metrics,
    msg: Msg<Ctx>,
    mut on_yield: impl FnMut(Effect<Ctx>) -> Resume<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let mut co = Co::new(|yielder, start| {
        debug_assert!(matches!(start, Resume::Start));
        handle_msg(state, metrics, yielder, msg)
    });

    let mut co_result = co.resume(Resume::Start);
    loop {
        match co_result {
            CoResult::Yield(yld) => co_result = co.resume(on_yield(yld)),
            CoResult::Return(result) => return result,
        }
    }
}

/// Process a message asynchronously.
///
/// # Example
/// TODO
pub async fn process_async<'a, Ctx>(
    state: &'a mut State<Ctx>,
    metrics: &'a Metrics,
    msg: Msg<Ctx>,
    mut on_yield: impl FnMut(Effect<Ctx>) -> Pin<Box<dyn Future<Output = Resume<Ctx>>>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let mut co = Co::new(|yielder, start| {
        debug_assert!(matches!(start, Resume::Start));
        handle_msg(state, metrics, yielder, msg)
    });

    let mut co_result = co.resume(Resume::Start);
    loop {
        match co_result {
            CoResult::Yield(yld) => co_result = co.resume(on_yield(yld).await),
            CoResult::Return(result) => return result,
        }
    }
}

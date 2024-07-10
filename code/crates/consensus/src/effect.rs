use corosensei::stack::DefaultStack;
use corosensei::ScopedCoroutine;
use derive_where::derive_where;

use malachite_common::*;

use crate::Error;

pub type Co<'a, Ctx> =
    ScopedCoroutine<'a, Resume<Ctx>, Effect<Ctx>, Result<(), Error<Ctx>>, DefaultStack>;
pub type CoResult<Ctx> = corosensei::CoroutineResult<Effect<Ctx>, Result<(), Error<Ctx>>>;
pub type Yielder<Ctx> = corosensei::Yielder<Resume<Ctx>, Effect<Ctx>>;

#[must_use]
#[derive_where(Debug)]
pub enum Effect<Ctx>
where
    Ctx: Context,
{
    /// Reset all timeouts
    /// Resume with: Resume::Continue
    ResetTimeouts,

    /// Cancel all timeouts
    /// Resume with: Resume::Continue
    CancelAllTimeouts,

    /// Cancel a given timeout
    /// Resume with: Resume::Continue
    CancelTimeout(Timeout),

    /// Schedule a timeout
    /// Resume with: Resume::Continue
    ScheduleTimeout(Timeout),

    /// Broadcast a message
    /// Resume with: Resume::Continue
    Broadcast(),

    /// Get a value to propose at the given height and round, within the given timeout
    /// Resume with: Resume::ProposeValue(height, round, value)
    GetValue(Ctx::Height, Round, Timeout),

    /// Get the validator set at the given height
    /// Resume with: Resume::ValidatorSet(height, validator_set)
    GetValidatorSet(Ctx::Height),

    /// Consensus has decided on a value
    /// Resume with: Resume::Continue
    DecidedOnValue {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedVote<Ctx>>,
    },
}

#[must_use]
#[derive_where(Debug)]
pub enum Resume<Ctx>
where
    Ctx: Context,
{
    Start,
    Continue,
    ProposeValue(Ctx::Height, Round, Ctx::Value),
    ValidatorSet(Ctx::Height, Ctx::ValidatorSet),
}

#[macro_export]
macro_rules! emit {
    ($yielder:expr, $effect:expr) => {
        emit_then!($yielder, $effect, $crate::handle::Resume::Continue)
    };
}

#[macro_export]
macro_rules! emit_then {
    ($yielder:expr, $effect:expr, $pat:pat) => {
        emit_then!($yielder, $effect, $pat => ())
    };

    // TODO: Add support for if guards
    ($yielder:expr, $effect:expr $(, $pat:pat => $expr:expr)+ $(,)*) => {
        match $yielder.suspend($effect) {
            $($pat => $expr,)+
            resume => {
                return Err($crate::error::Error::UnexpectedResume(
                    resume,
                    concat!(concat!($(stringify!($pat))+), ", ")
                )
                .into())
            }
        }
    };
}

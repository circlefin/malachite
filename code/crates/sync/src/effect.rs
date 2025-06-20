use std::marker::PhantomData;

use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::Context;
use malachitebft_peer::PeerId;

use crate::{InboundRequestId, OutboundRequestId, ValueRequest, ValueResponse};

/// Provides a way to construct the appropriate [`Resume`] value to
/// resume execution after handling an [`Effect`].
///
/// Each `Effect` embeds a value that implements [`Resumable`]
/// which is used to construct the appropriate [`Resume`] value.
///
/// ## Example
///
/// ```rust,ignore
/// fn effect_handler(effect: Effect<Ctx>) -> Result<Resume<Ctx>, Error> {
///     match effect {
///         Effect::ResetTimeouts(r) => {
///             reset_timeouts();
///             Ok(r.resume_with(()))
///         }
///         Effect::GetValidatorSet(height, r) => {
///             let validator_set = get_validator_set(height);
///             Ok(r.resume_with(validator_set))
///         }
///        // ...
///     }
/// }
/// ```
pub trait Resumable<Ctx: Context> {
    /// The value type that will be used to resume execution
    type Value;

    /// Creates the appropriate [`Resume`] value to resume execution with.
    fn resume_with(self, value: Self::Value) -> Resume<Ctx>;
}

#[derive_where(Debug)]
#[derive(Error)]
pub enum Error<Ctx: Context> {
    /// The coroutine was resumed with a value which
    /// does not match the expected type of resume value.
    #[error("Unexpected resume: {0:?}, expected one of: {1}")]
    UnexpectedResume(Resume<Ctx>, &'static str),
}

#[derive_where(Debug)]
pub enum Resume<Ctx: Context> {
    Continue(PhantomData<Ctx>),
    ValueRequestId(Option<OutboundRequestId>),
}

impl<Ctx: Context> Default for Resume<Ctx> {
    fn default() -> Self {
        Self::Continue(PhantomData)
    }
}

#[derive_where(Debug)]
pub enum Effect<Ctx: Context> {
    /// Broadcast our status to our direct peers
    BroadcastStatus(Ctx::Height, resume::Continue),

    /// Send a ValueSync request to a peer
    SendValueRequest(PeerId, ValueRequest<Ctx>, resume::ValueRequestId),

    /// Send a response to a ValueSync request
    SendValueResponse(InboundRequestId, ValueResponse<Ctx>, resume::Continue),

    /// Retrieve a value from the application
    GetDecidedValue(InboundRequestId, Ctx::Height, resume::Continue),
}

pub mod resume {

    use super::*;

    #[derive(Debug, Default)]
    pub struct Continue;

    impl<Ctx: Context> Resumable<Ctx> for Continue {
        type Value = ();

        fn resume_with(self, _: ()) -> Resume<Ctx> {
            Resume::default()
        }
    }

    #[derive(Debug, Default)]
    pub struct ValueRequestId;

    impl<Ctx: Context> Resumable<Ctx> for ValueRequestId {
        type Value = Option<OutboundRequestId>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::ValueRequestId(value)
        }
    }
}

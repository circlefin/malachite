use derive_where::derive_where;
use displaydoc::Display;

use malachite_common::{Context, Round};
use malachite_driver::Error as DriverError;

use crate::effect::Resume;

/// The types of error that can be emitted by the consensus process.
#[derive(Display)]
#[derive_where(Debug)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// Unexpected resume: {0:?}, expected one of: {1}"
    UnexpectedResume(Resume<Ctx>, &'static str),

    /// Proposer not found at height {0} and round {1}
    ProposerNotFound(Ctx::Height, Round),

    /// Driver failed to process input, reason: {0}
    DriverProcess(DriverError<Ctx>),
}

impl<Ctx: Context> core::error::Error for Error<Ctx> {}

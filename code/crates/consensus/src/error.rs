use derive_where::derive_where;
use displaydoc::Display;

use malachite_common::{Context, Round};
use malachite_driver::Error as DriverError;

/// The types of error that can be emitted by the consensus process.
#[derive(Display)]
#[derive_where(Debug)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// Proposer not found at height {0} and round {1}
    ProposerNotFound(Ctx::Height, Round),

    /// Decided value not found after commit timeout at height {0} and round {1}
    DecidedValueNotFound(Ctx::Height, Round),

    /// Driver failed to process input: {0}
    DriverProcess(DriverError<Ctx>),
}

impl<Ctx: Context> core::error::Error for Error<Ctx> {}

use derive_where::derive_where;

use malachite_common::{Context, Round};
use malachite_driver::Error as DriverError;

use crate::effect::Resume;

/// The types of error that can be emitted by the consensus process.
#[derive_where(Debug)]
#[derive(thiserror::Error)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// The consensus process was resumed with a value which
    /// does not match the expected type of resume value.
    #[error("Unexpected resume: {0:?}, expected one of: {1}")]
    UnexpectedResume(Resume<Ctx>, &'static str),

    /// The proposer was not found at the given height and round.
    #[error("Proposer not found at height {0} and round {1}")]
    ProposerNotFound(Ctx::Height, Round),

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),
}

use malachite_common::{Context, Round};
use malachite_driver::Error as DriverError;

use crate::effect::Resume;

#[derive(Debug, thiserror::Error)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    #[error("Proposer not found at height {0} and round {1}")]
    ProposerNotFound(Ctx::Height, Round),

    #[error("Unexpected resume: {0:?}, expected one of: {1}")]
    UnexpectedResume(Resume<Ctx>, &'static str),

    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),
}

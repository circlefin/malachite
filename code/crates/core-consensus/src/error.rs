use derive_where::derive_where;

use malachitebft_core_driver::Error as DriverError;
use malachitebft_core_types::{Context, Round};

use crate::effect::Resume;

/// The types of error that can be emitted by the consensus process.
#[derive_where(Debug)]
#[derive(thiserror::Error)]
#[allow(private_interfaces)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// The consensus process was resumed with a value which
    /// does not match the expected type of resume value.
    #[allow(private_interfaces)]
    #[error("Unexpected resume: {0:?}, expected one of: {1}")]
    UnexpectedResume(Resume<Ctx>, &'static str),

    /// The proposer was not found at the given height and round.
    #[error("Proposer not found at height {0} and round {1}")]
    ProposerNotFound(Ctx::Height, Round),

    /// State machine has no decision in commit step.
    #[error("State machine has no decision in commit step")]
    DecisionNotFound(Ctx::Height, Round),

    /// Driver proposal not found in commit step.
    #[error("Driver proposal not found in commit step")]
    DriverProposalNotFound(Ctx::Height, Round),

    /// Full proposal not found in commit step.
    #[error("Full proposal not found in commit step")]
    FullProposalNotFound(Ctx::Height, Round),

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),
}

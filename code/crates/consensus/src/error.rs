use derive_where::derive_where;

use malachite_common::{Context, Round};
use malachite_driver::Error as DriverError;

/// The types of error that can be emitted by the consensus process.
#[derive_where(Debug)]
#[derive(thiserror::Error)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// The proposer was not found at the given height and round.
    #[error("Proposer not found at height {0} and round {1}")]
    ProposerNotFound(Ctx::Height, Round),

    /// Decided value not found after commit timeout.
    #[error("Decided value not found after commit timeout")]
    DecidedValueNotFound(Ctx::Height, Round),

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),
}

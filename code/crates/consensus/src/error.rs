use derive_where::derive_where;

use malachite_core_driver::Error as DriverError;
use malachite_core_types::{CertificateError, CommitCertificate, Context, Round};

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

    /// Decided value not found after commit timeout.
    #[error("Decided value not found after commit timeout")]
    DecidedValueNotFound(Ctx::Height, Round),

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),

    /// The validator set was not found at the given height.
    #[error("Validator set not found at height {0}")]
    ValidatorSetNotFound(Ctx::Height),

    /// The certificate is invalid.
    #[error("Invalid certificate: {1}")]
    InvalidCertificate(CommitCertificate<Ctx>, CertificateError<Ctx>),
}

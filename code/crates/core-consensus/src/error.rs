use std::io;
use std::sync::Arc;

use derive_where::derive_where;

use malachitebft_core_driver::Error as DriverError;
use malachitebft_core_types::{
    CertificateError, Context, ExtendedCommitCertificate, Round, ValueId,
};

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

    /// State machine has no decision in commit step.
    #[error("State machine has no decision in commit step")]
    DecisionNotFound(Ctx::Height, Round),

    /// The driver failed to process an input.
    #[error("Driver failed to process input, reason: {0}")]
    DriverProcess(DriverError<Ctx>),

    /// The certificate is invalid — either a precommit signature, the 2/3+
    /// quorum, or a vote-extension signature failed to verify.
    #[error("Invalid certificate: {1}")]
    InvalidCommitCertificate(ExtendedCommitCertificate<Ctx>, CertificateError<Ctx>),

    /// Missing polka certificate.
    #[error("Missing polka certificate at height {0}, round {1}, value {2}, for {3}")]
    MissingPolkaCertificate(Ctx::Height, Round, ValueId<Ctx>, &'static str),

    /// The application did not supply an extension for a non-nil precommit at a
    /// height where vote extensions are required.
    #[error("Vote extension required at height {0}, round {1}, value {2}")]
    VoteExtensionRequired(Ctx::Height, Round, ValueId<Ctx>),

    /// The write-ahead log is corrupted.
    #[error("Write-ahead log is corrupted: {0}")]
    WalCorrupted(Arc<io::Error>),
}

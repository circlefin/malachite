use std::io;
use std::sync::Arc;

use derive_where::derive_where;

use malachitebft_core_driver::Error as DriverError;
use malachitebft_core_types::{
    CertificateError, Context, ExtendedCommitCertificate, Round, ValueId,
};

use crate::effect::Resume;
use crate::wal_replay_index::RecordKind;

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

    /// The certificate is invalid — a precommit signature, the 2/3+ quorum, a
    /// vote-extension signature, or the application's vote-extension check failed.
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

    /// Replaying the write-ahead log re-derived a message that disagrees with the one recorded
    /// for the same kind, height and round.
    #[error("Write-ahead log replay re-derived a {0} at height {1}, round {2} that does not match the recorded one")]
    ReplayDivergence(RecordKind, Ctx::Height, Round),
}

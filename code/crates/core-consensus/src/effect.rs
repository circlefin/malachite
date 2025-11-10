use derive_where::derive_where;

use malachitebft_core_types::*;

use crate::types::{LivenessMsg, SignedConsensusMsg};
use crate::{ConsensusMsg, Error, PeerId, Role, VoteExtensionError, WalEntry};

/// Provides a way to construct the appropriate [`Resume`] value to
/// resume execution after handling an [`Effect`].
///
/// Eeach `Effect` embeds a value that implements [`Resumable`]
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

/// An effect which may be yielded by a consensus process.
///
/// Effects are handled by the caller using [`process!`][process]
/// and the consensus process is then resumed with an appropriate [`Resume`] value, as per
/// the documentation for each effect.
///
/// [process]: crate::process
#[must_use]
#[derive_where(Debug)]
pub enum Effect<Ctx>
where
    Ctx: Context,
{
    /// Reset all timeouts to their initial values
    ///
    /// Resume with: [`resume::Continue`]
    ResetTimeouts(resume::Continue),

    /// Cancel all outstanding timeouts
    ///
    /// Resume with: [`resume::Continue`]
    CancelAllTimeouts(resume::Continue),

    /// Cancel a given timeout
    ///
    /// Resume with: [`resume::Continue`]
    CancelTimeout(Timeout, resume::Continue),

    /// Schedule a timeout
    ///
    /// Resume with: [`resume::Continue`]
    ScheduleTimeout(Timeout, resume::Continue),

    /// Consensus is starting a new round with the given proposer
    ///
    /// Resume with: [`resume::Continue`]
    StartRound(Ctx::Height, Round, Ctx::Address, Role, resume::Continue),

    /// Publish a message to peers
    ///
    /// Resume with: [`resume::Continue`]
    PublishConsensusMsg(SignedConsensusMsg<Ctx>, resume::Continue),

    /// Publish a liveness message to peers
    ///
    /// Resume with: [`resume::Continue`]
    PublishLivenessMsg(LivenessMsg<Ctx>, resume::Continue),

    /// Re-publish a vote to peers
    ///
    /// Resume with: [`resume::Continue`]
    RepublishVote(SignedVote<Ctx>, resume::Continue),

    /// Re-publish a round certificate to peers
    ///
    /// Resume with: [`resume::Continue`]
    RepublishRoundCertificate(RoundCertificate<Ctx>, resume::Continue),

    /// Requests the application to build a value for consensus to run on.
    ///
    /// Because this operation may be asynchronous, this effect does not expect a resumption
    /// with a value, rather the application is expected to propose a value within the timeout duration.
    ///
    /// The application MUST eventually feed a [`Propose`][crate::input::Input::Propose]
    /// input to consensus within the specified timeout duration.
    ///
    /// Resume with: [`resume::Continue`]
    GetValue(Ctx::Height, Round, Timeout, resume::Continue),

    /// Requests the application to re-stream a proposal that it has already seen.
    ///
    /// The application MUST re-publish again to its peers all
    /// the proposal parts pertaining to that value.
    ///
    /// Resume with: [`resume::Continue`]
    RestreamProposal(
        /// Height of the value
        Ctx::Height,
        /// Round of the value
        Round,
        /// Valid round of the value
        Round,
        /// Address of the proposer for that value
        Ctx::Address,
        /// Value ID of the value to restream
        ValueId<Ctx>,
        /// For resumption
        resume::Continue,
    ),

    /// Notifies the application that consensus has received a valid sync value response.
    ///
    /// Resume with: [`resume::Continue`]
    ValidSyncValue(
        /// The value response
        ValueResponse<Ctx>,
        /// The proposer for that value
        Ctx::Address,
        /// How to resume
        resume::Continue,
    ),

    /// Notifies the engine that consensus has received an invalid sync value response.
    ///
    /// Resume with: [`resume::Continue`]
    InvalidSyncValue(
        /// The peer that sent the invalid response
        PeerId,
        /// The height for which the response was sent
        Ctx::Height,
        /// The error that was encountered
        Error<Ctx>,
        /// How to resume
        resume::Continue,
    ),

    /// Notifies the application that consensus has decided on a value.
    ///
    /// This message includes a commit certificate containing the ID of
    /// the value that was decided on, the height and round at which it was decided,
    /// and the aggregated signatures of the validators that committed to it.
    ///
    /// It also includes the vote extensions that were received for this height.
    ///
    /// Resume with: [`resume::Continue`]
    Decide(
        CommitCertificate<Ctx>,
        VoteExtensions<Ctx>,
        resume::Continue,
    ),

    /// Sign a vote with this node's private key
    ///
    /// Resume with: [`resume::SignedVote`]
    SignVote(Ctx::Vote, resume::SignedVote),

    /// Sign a proposal with this node's private key
    ///
    /// Resume with: [`resume::SignedProposal`]
    SignProposal(Ctx::Proposal, resume::SignedProposal),

    /// Verify a signature
    ///
    /// Resume with: [`resume::SignatureValidity`]
    VerifySignature(
        SignedMessage<Ctx, ConsensusMsg<Ctx>>,
        PublicKey<Ctx>,
        resume::SignatureValidity,
    ),

    /// Verify a commit certificate
    ///
    /// Resume with: [`resume::CertificateValidity`]
    VerifyCommitCertificate(
        CommitCertificate<Ctx>,
        Ctx::ValidatorSet,
        ThresholdParams,
        resume::CertificateValidity,
    ),

    /// Verify a polka certificate
    ///
    /// Resume with: [`resume::CertificateValidity`]
    VerifyPolkaCertificate(
        PolkaCertificate<Ctx>,
        Ctx::ValidatorSet,
        ThresholdParams,
        resume::CertificateValidity,
    ),

    /// Verify a round certificate
    ///
    /// Resume with: [`resume::CertificateValidity`]
    VerifyRoundCertificate(
        RoundCertificate<Ctx>,
        Ctx::ValidatorSet,
        ThresholdParams,
        resume::CertificateValidity,
    ),

    /// Append an entry to the Write-Ahead Log for crash recovery
    /// If the WAL is not at the given height, the entry should be ignored.
    ///
    /// Resume with: [`resume::Continue`]`
    WalAppend(Ctx::Height, WalEntry<Ctx>, resume::Continue),

    /// Allows the application to extend the pre-commit vote with arbitrary data.
    ///
    /// When consensus is preparing to send a pre-commit vote, it first calls `ExtendVote`.
    /// The application then returns a blob of data called a vote extension.
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    /// The proposer of the next block will receive all vote extensions along with the commit certificate.
    ///
    /// Only emitted if vote extensions are enabled.
    ExtendVote(Ctx::Height, Round, ValueId<Ctx>, resume::VoteExtension),

    /// Verify a vote extension
    ///
    /// If the vote extension is deemed invalid, the vote it was part of
    /// will be discarded altogether.
    ///
    ///
    /// Only emitted if vote extensions are enabled.
    ///
    /// Resume with: [`resume::VoteExtensionValidity`]
    VerifyVoteExtension(
        Ctx::Height,
        Round,
        ValueId<Ctx>,
        SignedExtension<Ctx>,
        PublicKey<Ctx>,
        resume::VoteExtensionValidity,
    ),
}

/// A value with which the consensus process can be resumed after yielding an [`Effect`].
#[must_use]
#[allow(clippy::manual_non_exhaustive)]
#[derive_where(Debug)]
pub enum Resume<Ctx>
where
    Ctx: Context,
{
    /// Internal effect to start the coroutine.
    #[doc(hidden)]
    Start,

    /// Resume execution
    Continue,

    /// Resume execution with `Some(Ctx::ValidatorSet)` if a validator set
    /// was successfully fetched, or `None` otherwise.
    ValidatorSet(Option<Ctx::ValidatorSet>),

    /// Resume execution with the validity of the signature
    SignatureValidity(bool),

    /// Resume execution with the signed vote
    SignedVote(SignedMessage<Ctx, Ctx::Vote>),

    /// Resume execution with the signed proposal
    SignedProposal(SignedMessage<Ctx, Ctx::Proposal>),

    /// Resume with an optional vote extension.
    /// See the [`Effect::ExtendVote`] effect for more information.
    VoteExtension(Option<SignedExtension<Ctx>>),

    /// Resume execution with the result of the verification of the [`SignedExtension`]
    VoteExtensionValidity(Result<(), VoteExtensionError>),

    /// Resume execution with the result of the verification of the [`CommitCertificate`]
    CertificateValidity(Result<(), CertificateError<Ctx>>),
}

pub mod resume {
    use crate::VoteExtensionError;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Continue;

    impl<Ctx: Context> Resumable<Ctx> for Continue {
        type Value = ();

        fn resume_with(self, _: ()) -> Resume<Ctx> {
            Resume::Continue
        }
    }

    #[derive(Debug, Default)]
    pub struct SignatureValidity;

    impl<Ctx: Context> Resumable<Ctx> for SignatureValidity {
        type Value = bool;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::SignatureValidity(value)
        }
    }

    #[derive(Debug, Default)]
    pub struct SignedVote;

    impl<Ctx: Context> Resumable<Ctx> for SignedVote {
        type Value = SignedMessage<Ctx, Ctx::Vote>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::SignedVote(value)
        }
    }

    #[derive(Debug, Default)]
    pub struct SignedProposal;

    impl<Ctx: Context> Resumable<Ctx> for SignedProposal {
        type Value = SignedMessage<Ctx, Ctx::Proposal>;

        fn resume_with(self, a: Self::Value) -> Resume<Ctx> {
            Resume::SignedProposal(a)
        }
    }

    #[derive(Debug, Default)]
    pub struct VoteExtension;

    impl<Ctx: Context> Resumable<Ctx> for VoteExtension {
        type Value = Option<SignedExtension<Ctx>>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::VoteExtension(value)
        }
    }

    #[derive(Debug, Default)]
    pub struct CertificateValidity;

    impl<Ctx: Context> Resumable<Ctx> for CertificateValidity {
        type Value = Result<(), CertificateError<Ctx>>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::CertificateValidity(value)
        }
    }

    #[derive(Debug, Default)]
    pub struct VoteExtensionValidity;

    impl<Ctx: Context> Resumable<Ctx> for VoteExtensionValidity {
        type Value = Result<(), VoteExtensionError>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::VoteExtensionValidity(value)
        }
    }
}

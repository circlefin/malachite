use derive_where::derive_where;

use malachite_core_types::*;

use crate::input::RequestId;
use crate::types::SignedConsensusMsg;
use crate::ConsensusMsg;

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
/// match effect {
///    Effect::ResetTimeouts(r) => {
///      reset_timeouts();
///      Ok(r.resume_with(()))
///    }
///    Effect::GetValidatorSet(height, r) => {)
///        let validator_set = get_validator_set(height);
///        Ok(r.resume_with(validator_set))
///    }
///    // ...
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
    /// Reset all timeouts
    /// Resume with: [`Resume::Continue`]
    ResetTimeouts(resume::Continue),

    /// Cancel all timeouts
    /// Resume with: [`Resume::Continue`]
    CancelAllTimeouts(resume::Continue),

    /// Cancel a given timeout
    /// Resume with: [`Resume::Continue`]
    CancelTimeout(Timeout, resume::Continue),

    /// Schedule a timeout
    /// Resume with: [`Resume::Continue`]
    ScheduleTimeout(Timeout, resume::Continue),

    /// Consensus is starting a new round with the given proposer
    /// Resume with: [`Resume::Continue`]
    StartRound(Ctx::Height, Round, Ctx::Address, resume::Continue),

    /// Broadcast a message
    /// Resume with: [`Resume::Continue`]
    Broadcast(SignedConsensusMsg<Ctx>, resume::Continue),

    /// Get a value to propose at the given height and round, within the given timeout
    /// Resume with: [`Resume::Continue`]
    GetValue(Ctx::Height, Round, Timeout, resume::Continue),

    /// Restream the value identified by the given information.
    /// Resume with: [`Resume::Continue`]
    RestreamValue(
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

    /// Get the validator set at the given height
    /// Resume with: [`Resume::ValidatorSet`]
    GetValidatorSet(Ctx::Height, resume::ValidatorSet),

    /// Consensus has decided on a value
    /// Resume with: [`Resume::Continue`]
    Decide(CommitCertificate<Ctx>, resume::Continue),

    /// Consensus has been stuck in Prevote or Precommit step, ask for vote sets from peers
    /// Resume with: [`Resume::Continue`]
    GetVoteSet(Ctx::Height, Round, resume::Continue),

    /// A peer has required our vote set, send the response
    /// Resume with: [`Resume::Continue`]`
    SendVoteSetResponse(
        RequestId,
        Ctx::Height,
        Round,
        VoteSet<Ctx>,
        resume::Continue,
    ),

    /// Persist a consensus message in the Write-Ahead Log for crash recovery
    /// Resume with: [`Resume::Continue`]`
    PersistMessage(SignedConsensusMsg<Ctx>, resume::Continue),

    /// Persist a timeout in the Write-Ahead Log for crash recovery
    /// Resume with: [`Resume::Continue`]`
    PersistTimeout(Timeout, resume::Continue),

    /// Sign a vote with this node's private key
    /// Resume with: [`Resume::SignedVote`]
    SignVote(Ctx::Vote, resume::SignedVote),

    /// Sign a proposal with this node's private key
    /// Resume with: [`Resume::SignedProposal`]
    SignProposal(Ctx::Proposal, resume::SignedProposal),

    /// Verify a signature
    /// Resume with: [`Resume::SignatureValidity`]
    VerifySignature(
        SignedMessage<Ctx, ConsensusMsg<Ctx>>,
        PublicKey<Ctx>,
        resume::SignatureValidity,
    ),

    /// Verify a commit certificate
    VerifyCertificate(
        CommitCertificate<Ctx>,
        Ctx::ValidatorSet,
        ThresholdParams,
        resume::CertificateValidity,
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

    /// Resume execution with the result of the verification of the [`CommitCertificate`]
    CertificateValidity(Result<(), CertificateError<Ctx>>),
}

pub mod resume {
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
    pub struct ValidatorSet;

    impl<Ctx: Context> Resumable<Ctx> for ValidatorSet {
        type Value = Option<Ctx::ValidatorSet>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::ValidatorSet(value)
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
    pub struct CertificateValidity;

    impl<Ctx: Context> Resumable<Ctx> for CertificateValidity {
        type Value = Result<(), CertificateError<Ctx>>;

        fn resume_with(self, value: Self::Value) -> Resume<Ctx> {
            Resume::CertificateValidity(value)
        }
    }
}

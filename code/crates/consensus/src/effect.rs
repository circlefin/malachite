use derive_where::derive_where;

use malachite_core_types::*;

use crate::input::RequestId;
use crate::types::SignedConsensusMsg;
use crate::ConsensusMsg;

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
    ResetTimeouts,

    /// Cancel all timeouts
    /// Resume with: [`Resume::Continue`]
    CancelAllTimeouts,

    /// Cancel a given timeout
    /// Resume with: [`Resume::Continue`]
    CancelTimeout(Timeout),

    /// Schedule a timeout
    /// Resume with: [`Resume::Continue`]
    ScheduleTimeout(Timeout),

    /// Consensus is starting a new round with the given proposer
    /// Resume with: [`Resume::Continue`]
    StartRound(Ctx::Height, Round, Ctx::Address),

    /// Broadcast a message
    /// Resume with: [`Resume::Continue`]
    Broadcast(SignedConsensusMsg<Ctx>),

    /// Get a value to propose at the given height and round, within the given timeout
    /// Resume with: [`Resume::Continue`]
    GetValue(Ctx::Height, Round, Timeout),

    /// Restream value at the given height, round and valid round
    /// Resume with: [`Resume::Continue`]
    RestreamValue(Ctx::Height, Round, Round, Ctx::Address, ValueId<Ctx>),

    /// Get the validator set at the given height
    /// Resume with: [`Resume::ValidatorSet`]
    GetValidatorSet(Ctx::Height),

    /// Consensus has decided on a value
    /// Resume with: [`Resume::Continue`]
    Decide { certificate: CommitCertificate<Ctx> },

    /// Consensus has been stuck in Prevote or Precommit step, ask for vote sets from peers
    /// Resume with: [`Resume::Continue`]
    GetVoteSet(Ctx::Height, Round),

    /// A peer has required our vote set, send the response
    SendVoteSetResponse(RequestId, Ctx::Height, Round, VoteSet<Ctx>),

    /// Persist a consensus message in the Write-Ahead Log for crash recovery
    PersistMessage(SignedConsensusMsg<Ctx>),

    /// Persist a timeout in the Write-Ahead Log for crash recovery
    PersistTimeout(Timeout),

    /// Sign a vote with this node's private key
    /// Resume with: [`Resume::SignedVote`]
    SignVote(Ctx::Vote),

    /// Sign a proposal with this node's private key
    /// Resume with: [`Resume::SignedProposal`]
    SignProposal(Ctx::Proposal),

    /// Verify a signature
    /// Resume with: [`Resume::SignatureValidity`]
    VerifySignature(SignedMessage<Ctx, ConsensusMsg<Ctx>>, PublicKey<Ctx>),

    /// Verify a commit certificate
    VerifyCertificate(CommitCertificate<Ctx>, Ctx::ValidatorSet, ThresholdParams),
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

    /// Resume execution with an optional validator set at the given height
    ValidatorSet(Ctx::Height, Option<Ctx::ValidatorSet>),

    /// Resume execution with the validity of the signature
    SignatureValidity(bool),

    /// Resume execution with the signed vote
    SignedVote(SignedMessage<Ctx, Ctx::Vote>),

    /// Resume execution with the signed proposal
    SignedProposal(SignedMessage<Ctx, Ctx::Proposal>),

    /// Resume execution with the result of the verification of the [`CommitCertificate`]
    CertificateValidity(Result<(), CertificateError<Ctx>>),
}

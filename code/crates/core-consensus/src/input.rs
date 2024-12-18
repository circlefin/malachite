use derive_where::derive_where;
use malachitebft_core_types::{
    CommitCertificate, Context, Round, SignedProposal, SignedVote, Timeout, ValueOrigin, VoteSet,
};

use crate::types::ProposedValue;
use crate::ValueToPropose;

pub type RequestId = String;

/// Inputs to be handled by the consensus process.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start a new height with the given validator set
    StartHeight(Ctx::Height, Ctx::ValidatorSet),

    /// Process a vote
    Vote(SignedVote<Ctx>),

    /// Process a proposal
    Proposal(SignedProposal<Ctx>),

    /// Propose a value
    Propose(ValueToPropose<Ctx>),

    /// A timeout has elapsed
    TimeoutElapsed(Timeout),

    /// Received the full proposed value corresponding to a proposal.
    /// The origin denotes whether the value was received via consensus or Sync.
    ProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// Received a commit certificate from Sync
    CommitCertificate(CommitCertificate<Ctx>),

    /// Peer needs vote set
    VoteSetRequest(RequestId, Ctx::Height, Round),

    /// Vote set to be sent to peer
    VoteSetResponse(VoteSet<Ctx>),
}

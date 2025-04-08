use derive_where::derive_where;
use malachitebft_core_types::{
    CommitCertificate, Context, PolkaCertificate, Round, SignedProposal, SignedVote, Timeout,
    ValueOrigin, VoteSet,
};

use crate::types::ProposedValue;
use crate::LocallyProposedValue;

pub type RequestId = String;

/// Inputs to be handled by the consensus process.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start consensus for the given height with the given validator set
    StartHeight(Ctx::Height, Ctx::ValidatorSet),

    /// Process a vote received over the network.
    Vote(SignedVote<Ctx>),

    /// Process a Proposal message received over the network
    ///
    /// This input MUST only be provided when `ValuePayload` is set to `ProposalOnly` or `ProposalAndParts`,
    /// i.e. when consensus runs in a mode where the proposer sends a Proposal consensus message over the network.
    Proposal(SignedProposal<Ctx>),

    /// Propose the given value.
    ///
    /// This input MUST only be provided when we are the proposer for the current round.
    Propose(LocallyProposedValue<Ctx>),

    /// A timeout has elapsed.
    TimeoutElapsed(Timeout),

    /// We have received the full proposal for the current round.
    ///
    /// The origin denotes whether the value was received via consensus gossip or via the sync protocol.
    ProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// We have received a commit certificate via the sync protocol.
    CommitCertificate(CommitCertificate<Ctx>),

    /// A peer has requested a vote set for the given height and round
    VoteSetRequest(RequestId, Ctx::Height, Round),

    /// Received a vote set from a peer via Sync
    VoteSetResponse(VoteSet<Ctx>, Vec<PolkaCertificate<Ctx>>),
}

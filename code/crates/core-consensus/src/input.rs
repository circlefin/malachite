use derive_where::derive_where;
use malachitebft_core_types::{
    Context, PolkaCertificate, RoundCertificate, SignedProposal, SignedVote, Timeout, ValueOrigin,
    ValueResponse,
};

use crate::types::{LocallyProposedValue, ProposedValue};

/// Inputs to be handled by the consensus process.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start consensus for the given height with an optional validator set update.
    /// The boolean indicates whether this is a restart of consensus for the given height.
    StartHeight(Ctx::Height, Option<Ctx::ValidatorSet>, bool),

    /// Process a vote received over the network.
    Vote(SignedVote<Ctx>),

    /// Process a Proposal message received over the network
    ///
    /// This input MUST only be provided when `ValuePayload` is set to `ProposalOnly` or `ProposalAndParts`,
    /// i.e. when consensus runs in a mode where the proposer sends a Proposal consensus message over the network.
    Proposal(SignedProposal<Ctx>),

    /// Process a PolkaCertificate message received over the network
    PolkaCertificate(PolkaCertificate<Ctx>),

    /// Process a RoundCertificate message received over the network
    RoundCertificate(RoundCertificate<Ctx>),

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

    /// We have received a synced value via the sync protocol.
    SyncValueResponse(ValueResponse<Ctx>),
}

use derive_where::derive_where;

use malachite_common::{CommitCertificate, Context, SignedProposal, SignedVote, Timeout};

use crate::types::ProposedValue;
use crate::ValueToPropose;

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

    /// Received the full proposed value corresponding to a proposal
    ProposedValue(ProposedValue<Ctx>),

    /// Received a commit certificate from BlockSync
    CommitCertificate(CommitCertificate<Ctx>),
}

use derive_where::derive_where;

use crate::{Context, Signature};

/// A signed proposal, ie. a proposal emitted by a validator and signed by its private key.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SignedProposal<Ctx>
where
    Ctx: Context,
{
    /// The proposal.
    pub proposal: Ctx::Proposal,

    /// The signature of the proposal.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedProposal<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed proposal from the given proposal and signature.
    pub fn new(proposal: Ctx::Proposal, signature: Signature<Ctx>) -> Self {
        Self {
            proposal,
            signature,
        }
    }
}

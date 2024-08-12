use derive_where::derive_where;

use crate::{Context, ProposalPart, Signature};

/// Defines the requirements for a signed proposal part type.

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SignedProposalPart<Ctx>
where
    Ctx: Context,
{
    /// The proposal part.
    pub proposal_part: Ctx::ProposalPart,

    /// The signature of the proposal part.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedProposalPart<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed proposal part from the given part and signature.
    pub fn new(proposal_part: Ctx::ProposalPart, signature: Signature<Ctx>) -> Self {
        Self {
            proposal_part,
            signature,
        }
    }
    /// Return the address of the validator that emitted this proposal part.
    pub fn validator_address(&self) -> &Ctx::Address {
        self.proposal_part.validator_address()
    }
}

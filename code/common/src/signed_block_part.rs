use crate::{Context, Signature};
use derive_where::derive_where;

/// Defines the requirements for a signed block part type.

#[derive_where(Debug, PartialEq, Eq)]
pub struct SignedBlockPart<Ctx>
where
    Ctx: Context,
{
    /// The block part.
    pub block_part: Ctx::BlockPart,

    /// The signature of the proposal.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedBlockPart<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed proposal from the given proposal and signature.
    pub fn new(block_part: Ctx::BlockPart, signature: Signature<Ctx>) -> Self {
        Self {
            block_part,
            signature,
        }
    }
}

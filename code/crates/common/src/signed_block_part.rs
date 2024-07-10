use derive_where::derive_where;

use crate::{BlockPart, Context, Signature};

/// Defines the requirements for a signed block part type.

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SignedBlockPart<Ctx>
where
    Ctx: Context,
{
    /// The block part.
    pub block_part: Ctx::BlockPart,

    /// The signature of the block part.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedBlockPart<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed block part from the given part and signature.
    pub fn new(block_part: Ctx::BlockPart, signature: Signature<Ctx>) -> Self {
        Self {
            block_part,
            signature,
        }
    }
    /// Return the address of the validator that emitted this block part.
    pub fn validator_address(&self) -> &Ctx::Address {
        self.block_part.validator_address()
    }
}

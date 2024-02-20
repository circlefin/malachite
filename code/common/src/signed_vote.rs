use derive_where::derive_where;

use crate::{Context, Signature, Vote};

/// A signed vote, ie. a vote emitted by a validator and signed by its private key.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<Ctx>
where
    Ctx: Context,
{
    /// The vote.
    pub vote: Ctx::Vote,

    /// The signature of the vote.
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedVote<Ctx>
where
    Ctx: Context,
{
    /// Create a new signed vote from the given vote and signature.
    pub fn new(vote: Ctx::Vote, signature: Signature<Ctx>) -> Self {
        Self { vote, signature }
    }

    /// Return the address of the validator that emitted this vote.
    pub fn validator_address(&self) -> &Ctx::Address {
        self.vote.validator_address()
    }
}

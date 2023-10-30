use crate::{Context, Signature};

// TODO: Do we need to abstract over `SignedVote` as well?

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<Ctx>
where
    Ctx: Context,
{
    pub vote: Ctx::Vote,
    pub address: Ctx::Address,
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedVote<Ctx>
where
    Ctx: Context,
{
    pub fn new(vote: Ctx::Vote, address: Ctx::Address, signature: Signature<Ctx>) -> Self {
        Self {
            vote,
            address,
            signature,
        }
    }
}

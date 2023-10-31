use crate::{Context, Signature, Vote};

// TODO: Do we need to abstract over `SignedVote` as well?

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<Ctx>
where
    Ctx: Context,
{
    pub vote: Ctx::Vote,
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedVote<Ctx>
where
    Ctx: Context,
{
    pub fn new(vote: Ctx::Vote, signature: Signature<Ctx>) -> Self {
        Self { vote, signature }
    }

    pub fn validator_address(&self) -> &Ctx::Address {
        self.vote.validator_address()
    }
}

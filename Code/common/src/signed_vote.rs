use crate::{Consensus, Signature};

// TODO: Do we need to abstract over `SignedVote` as well?

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<C>
where
    C: Consensus,
{
    pub vote: C::Vote,
    pub address: C::Address,
    pub signature: Signature<C>,
}

impl<C> SignedVote<C>
where
    C: Consensus,
{
    pub fn new(vote: C::Vote, address: C::Address, signature: Signature<C>) -> Self {
        Self {
            vote,
            address,
            signature,
        }
    }
}

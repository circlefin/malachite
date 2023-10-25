use malachite_common::Consensus;

// TODO: Do we need to abstract over `SignedVote` as well?

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedVote<C>
where
    C: Consensus,
{
    pub vote: C::Vote,
    pub address: C::Address,
    // TODO
    // pub signature: C::Signature,
}

impl<C> SignedVote<C>
where
    C: Consensus,
{
    pub fn new(vote: C::Vote, address: C::Address) -> Self {
        Self { vote, address }
    }
}

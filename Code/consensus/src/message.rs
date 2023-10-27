use malachite_common::{Consensus, Round, SignedVote, Timeout};

/// Messages that can be received and broadcast by the consensus executor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<C>
where
    C: Consensus,
{
    NewRound(Round),
    Proposal(C::Proposal),
    Vote(SignedVote<C>),
    Timeout(Timeout),
}

use malachite_common::{Consensus, Round, Timeout};

use crate::signed_vote::SignedVote;

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

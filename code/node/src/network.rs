use core::fmt;

pub mod broadcast;
mod msg;

use malachite_proto::{SignedProposal, SignedVote};

pub use self::msg::Msg;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(String);

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[allow(async_fn_in_trait)]
pub trait Network {
    async fn recv(&mut self) -> Option<(PeerId, Msg)>;
    async fn broadcast(&mut self, msg: Msg);

    async fn broadcast_vote(&mut self, vote: SignedVote) {
        self.broadcast(Msg::Vote(vote)).await
    }
    async fn broadcast_proposal(&mut self, proposal: SignedProposal) {
        self.broadcast(Msg::Proposal(proposal)).await
    }
}

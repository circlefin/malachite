use core::fmt;

use malachite_common::Context;

pub mod broadcast;
mod msg;

pub use self::msg::Msg;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(String);

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[allow(async_fn_in_trait)]
pub trait Network<Ctx: Context> {
    async fn recv(&mut self) -> Option<(PeerId, Msg<Ctx>)>;
    async fn broadcast(&mut self, msg: Msg<Ctx>);

    async fn broadcast_vote(&mut self, vote: Ctx::Vote) {
        self.broadcast(Msg::Vote(vote)).await
    }
    async fn broadcast_proposal(&mut self, proposal: Ctx::Proposal) {
        self.broadcast(Msg::Proposal(proposal)).await
    }
}

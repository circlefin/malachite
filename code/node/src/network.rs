use malachite_common::Context;

pub mod broadcast;
mod msg;

pub use self::msg::Msg;

#[allow(async_fn_in_trait)]
pub trait Network<Ctx: Context> {
    async fn recv(&mut self) -> Option<Msg<Ctx>>;
    async fn broadcast(&mut self, msg: Msg<Ctx>);

    async fn broadcast_vote(&mut self, vote: Ctx::Vote) {
        self.broadcast(Msg::Vote(vote)).await
    }
    async fn broadcast_proposal(&mut self, proposal: Ctx::Proposal) {
        self.broadcast(Msg::Proposal(proposal)).await
    }
}

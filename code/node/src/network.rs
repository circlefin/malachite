use core::fmt;
use std::convert::Infallible;
use std::str::FromStr;

use async_trait::async_trait;

pub mod broadcast;
mod msg;

use malachite_proto::{SignedProposal, SignedVote};

pub use self::msg::Msg;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(String);

impl PeerId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for PeerId {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[async_trait]
pub trait Network
where
    Self: Send + Sync + 'static,
{
    async fn recv(&mut self) -> Option<(PeerId, Msg)>;
    async fn broadcast(&mut self, msg: Msg);

    async fn broadcast_vote(&mut self, vote: SignedVote) {
        self.broadcast(Msg::Vote(vote)).await
    }
    async fn broadcast_proposal(&mut self, proposal: SignedProposal) {
        self.broadcast(Msg::Proposal(proposal)).await
    }
}

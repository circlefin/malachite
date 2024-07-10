use malachite_common::*;

use crate::mock::{Block, GossipEvent, Multiaddr, NetworkMsg, PeerId};

#[derive(Debug)]
pub enum Event<Ctx>
where
    Ctx: Context,
{
    Listening(Multiaddr),
    Message(PeerId, NetworkMsg<Ctx>),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

pub enum Msg<Ctx>
where
    Ctx: Context,
{
    /// Start a new height
    StartHeight(Ctx::Height),

    /// Move to a give height
    MoveToHeight(Ctx::Height),

    /// Process a gossip event
    GossipEvent(GossipEvent<Ctx>),

    /// A timeout has elapsed
    TimeoutElapsed(Timeout),

    /// A block to propose has been received
    ReceivedBlock(Block<Ctx>),
}

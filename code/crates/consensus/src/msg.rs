use malachite_common::*;

use crate::mock::{GossipEvent, Multiaddr, NetworkMsg, PeerId, ReceivedProposedValue};

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

    // The proposal builder has build a new block part, needs to be signed and gossiped by consensus
    GossipBlockPart(Ctx::BlockPart),

    /// A proposal has been received
    ProposalReceived(ReceivedProposedValue<Ctx>),
}

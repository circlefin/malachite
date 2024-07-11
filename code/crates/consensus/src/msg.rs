use malachite_common::*;

use crate::types::{Block, GossipEvent};

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

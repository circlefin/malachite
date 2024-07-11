use malachite_common::*;

use crate::types::{Block, GossipEvent};

/// Messages that can be handled by the consensus process
pub enum Msg<Ctx>
where
    Ctx: Context,
{
    /// Start a new height
    StartHeight(Ctx::Height),

    /// Process a gossip event
    GossipEvent(GossipEvent<Ctx>),

    /// Propose a value
    ProposeValue(Ctx::Height, Round, Ctx::Value),

    /// A timeout has elapsed
    TimeoutElapsed(Timeout),

    /// A block to propose has been received
    BlockReceived(Block<Ctx>),
}

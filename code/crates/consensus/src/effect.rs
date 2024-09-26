use derive_where::derive_where;

use malachite_common::*;

use crate::types::GossipMsg;

/// An effect which may be yielded by a consensus process.
///
/// Effects are handled by the caller using [`process`][process].
/// After that the consensus computation is then resumed.
///
/// [process]: crate::process
#[must_use]
#[derive_where(Debug)]
pub enum Effect<Ctx>
where
    Ctx: Context,
{
    /// Reset all timeouts
    ResetTimeouts,

    /// Cancel all timeouts
    CancelAllTimeouts,

    /// Cancel a given timeout
    CancelTimeout(Timeout),

    /// Schedule a timeout
    ScheduleTimeout(Timeout),

    /// Consensus is starting a new round with the given proposer
    StartRound(Ctx::Height, Round, Ctx::Address),

    /// Broadcast a message
    Broadcast(GossipMsg<Ctx>),

    /// Get a value to propose at the given height and round, within the given timeout
    GetValue(Ctx::Height, Round, Timeout),

    /// Consensus has decided on a value
    Decide {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedMessage<Ctx, Ctx::Vote>>,
    },
}

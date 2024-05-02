use derive_where::derive_where;

use malachite_common::{Context, Round, Timeout};

/// Messages emitted by the [`Driver`](crate::Driver)
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Output<Ctx>
where
    Ctx: Context,
{
    /// Start a new round
    NewRound(Ctx::Height, Round),

    /// Broadcast a proposal
    Propose(Ctx::Proposal),

    /// Broadcast a vote for a value
    Vote(Ctx::Vote),

    /// Decide on a value
    Decide(Round, Ctx::Value),

    /// Schedule a timeout
    ScheduleTimeout(Timeout),

    /// Ask for a value at the given height, round.
    /// The timeout tells the proposal builder how long it has to build a value.
    GetValue(Ctx::Height, Round, Timeout),
}

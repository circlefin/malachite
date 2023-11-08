use malachite_common::{Context, Round, SignedVote, Timeout};

/// Events that can be received by the [`Driver`](crate::Driver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<Ctx>
where
    Ctx: Context,
{
    /// A new round has started.
    /// The boolean indicates whether we are the proposer or not.
    NewRound(Round, bool),

    /// A new proposal has been received.
    Proposal(Ctx::Proposal),

    /// A new vote has been received.
    Vote(SignedVote<Ctx>),

    /// A timeout has elapsed.
    TimeoutElapsed(Timeout),
}

//! Inputs to the round state machine.

use derive_where::derive_where;

use malachitebft_core_types::{Context, Round, ValueId};

/// Input to the round state machine.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// No input
    NoInput,

    /// Start a new round, either as proposer or not.
    /// L14/L20
    NewRound(Round),

    /// Propose a value.
    /// L14
    ProposeValue(Ctx::Value),

    /// Receive a proposal.
    /// L22 + L23 (valid)
    Proposal(Ctx::Proposal),

    /// Receive an invalid proposal.
    /// L26 + L32 (invalid)
    InvalidProposal,

    /// Received a proposal and a polka value from a previous round.
    /// L28 + L29 (valid)
    ProposalAndPolkaPrevious(Ctx::Proposal),

    /// Received a proposal and a polka value from a previous round.
    /// L28 + L29 (invalid)
    InvalidProposalAndPolkaPrevious(Ctx::Proposal),

    /// Receive +2/3 prevotes for anything.
    /// L34
    PolkaAny,

    /// Receive +2/3 prevotes for nil.
    /// L44
    PolkaNil,

    /// Receive +2/3 prevotes for a value in current round.
    /// L36
    ProposalAndPolkaCurrent(Ctx::Proposal),

    /// Receive +2/3 precommits for anything.
    /// L47
    PrecommitAny,

    /// Receive +2/3 precommits for a value.
    /// L49
    ProposalAndPrecommitValue(Ctx::Proposal),

    /// Receive +2/3 precommits for a value.
    /// L51
    PrecommitValue(ValueId<Ctx>),

    /// Receive +1/3 messages from a higher round. OneCorrectProcessInHigherRound.
    /// L55
    SkipRound(Round),

    /// Timeout waiting for proposal.
    /// L57
    TimeoutPropose,

    /// Timeout waiting for prevotes.
    /// L61
    TimeoutPrevote,

    /// Timeout waiting for precommits.
    /// L65
    TimeoutPrecommit,
}

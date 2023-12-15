//! Inputs to the round state machine.

use core::fmt;

use malachite_common::{Context, Round, ValueId};

/// Input to the round state machine.
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start a new round, either as proposer or not.
    /// L14/L20
    NewRound,

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

impl<Ctx: Context> Clone for Input<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Input::NewRound => Input::NewRound,
            Input::ProposeValue(value) => Input::ProposeValue(value.clone()),
            Input::Proposal(proposal) => Input::Proposal(proposal.clone()),
            Input::InvalidProposal => Input::InvalidProposal,
            Input::ProposalAndPolkaPrevious(proposal) => {
                Input::ProposalAndPolkaPrevious(proposal.clone())
            }
            Input::InvalidProposalAndPolkaPrevious(proposal) => {
                Input::InvalidProposalAndPolkaPrevious(proposal.clone())
            }
            Input::PolkaAny => Input::PolkaAny,
            Input::PolkaNil => Input::PolkaNil,
            Input::ProposalAndPolkaCurrent(proposal) => {
                Input::ProposalAndPolkaCurrent(proposal.clone())
            }
            Input::PrecommitAny => Input::PrecommitAny,
            Input::ProposalAndPrecommitValue(proposal) => {
                Input::ProposalAndPrecommitValue(proposal.clone())
            }
            Input::PrecommitValue(value_id) => Input::PrecommitValue(value_id.clone()),
            Input::SkipRound(round) => Input::SkipRound(*round),
            Input::TimeoutPropose => Input::TimeoutPropose,
            Input::TimeoutPrevote => Input::TimeoutPrevote,
            Input::TimeoutPrecommit => Input::TimeoutPrecommit,
        }
    }
}

impl<Ctx: Context> PartialEq for Input<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Input::NewRound, Input::NewRound) => true,
            (Input::ProposeValue(value), Input::ProposeValue(other_value)) => value == other_value,
            (Input::Proposal(proposal), Input::Proposal(other_proposal)) => {
                proposal == other_proposal
            }
            (Input::InvalidProposal, Input::InvalidProposal) => true,
            (
                Input::ProposalAndPolkaPrevious(proposal),
                Input::ProposalAndPolkaPrevious(other_proposal),
            ) => proposal == other_proposal,
            (
                Input::InvalidProposalAndPolkaPrevious(proposal),
                Input::InvalidProposalAndPolkaPrevious(other_proposal),
            ) => proposal == other_proposal,
            (Input::PolkaAny, Input::PolkaAny) => true,
            (Input::PolkaNil, Input::PolkaNil) => true,
            (
                Input::ProposalAndPolkaCurrent(proposal),
                Input::ProposalAndPolkaCurrent(other_proposal),
            ) => proposal == other_proposal,
            (Input::PrecommitAny, Input::PrecommitAny) => true,
            (
                Input::ProposalAndPrecommitValue(proposal),
                Input::ProposalAndPrecommitValue(other_proposal),
            ) => proposal == other_proposal,
            (Input::PrecommitValue(value_id), Input::PrecommitValue(other_value_id)) => {
                value_id == other_value_id
            }
            (Input::SkipRound(round), Input::SkipRound(other_round)) => round == other_round,
            (Input::TimeoutPropose, Input::TimeoutPropose) => true,
            (Input::TimeoutPrevote, Input::TimeoutPrevote) => true,
            (Input::TimeoutPrecommit, Input::TimeoutPrecommit) => true,
            _ => false,
        }
    }
}

impl<Ctx: Context> Eq for Input<Ctx> {}

impl<Ctx> fmt::Debug for Input<Ctx>
where
    Ctx: Context,
    Ctx::Value: fmt::Debug,
    Ctx::Proposal: fmt::Debug,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Input::NewRound => write!(f, "NewRound"),
            Input::ProposeValue(value) => write!(f, "ProposeValue({:?})", value),
            Input::Proposal(proposal) => write!(f, "Proposal({:?})", proposal),
            Input::InvalidProposal => write!(f, "InvalidProposal"),
            Input::ProposalAndPolkaPrevious(proposal) => {
                write!(f, "ProposalAndPolkaPrevious({:?})", proposal)
            }
            Input::InvalidProposalAndPolkaPrevious(proposal) => {
                write!(f, "InvalidProposalAndPolkaPrevious({:?})", proposal)
            }
            Input::PolkaAny => write!(f, "PolkaAny"),
            Input::PolkaNil => write!(f, "PolkaNil"),
            Input::ProposalAndPolkaCurrent(proposal) => {
                write!(f, "ProposalAndPolkaCurrent({:?})", proposal)
            }
            Input::PrecommitAny => write!(f, "PrecommitAny"),
            Input::ProposalAndPrecommitValue(proposal) => {
                write!(f, "ProposalAndPrecommitValue({:?})", proposal)
            }
            Input::PrecommitValue(value_id) => write!(f, "PrecommitValue({:?})", value_id),
            Input::SkipRound(round) => write!(f, "SkipRound({:?})", round),
            Input::TimeoutPropose => write!(f, "TimeoutPropose"),
            Input::TimeoutPrevote => write!(f, "TimeoutPrevote"),
            Input::TimeoutPrecommit => write!(f, "TimeoutPrecommit"),
        }
    }
}

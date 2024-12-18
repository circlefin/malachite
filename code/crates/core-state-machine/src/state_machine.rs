//! The consensus state machine.

use malachitebft_core_types::{Context, NilOrVal, Proposal, Round, TimeoutKind, Value};

use crate::debug_trace;
use crate::input::Input;
use crate::output::Output;
use crate::state::{State, Step};
use crate::transition::Transition;

/// Immutable information about the input and our node:
/// - Address of our node
/// - Proposer for the round we are at
/// - Round for which the input is for, can be different than the round we are at
pub struct Info<'a, Ctx>
where
    Ctx: Context,
{
    /// The round for which the input is for, can be different than the round we are at
    pub input_round: Round,
    /// Address of our node
    pub address: &'a Ctx::Address,
    /// Proposer for the round we are at
    pub proposer: &'a Ctx::Address,
}

impl<'a, Ctx> Info<'a, Ctx>
where
    Ctx: Context,
{
    /// Create a new `Info` instance.
    pub fn new(input_round: Round, address: &'a Ctx::Address, proposer: &'a Ctx::Address) -> Self {
        Self {
            input_round,
            address,
            proposer,
        }
    }

    /// Create a new `Info` instance where we are the proposer.
    pub fn new_proposer(input_round: Round, address: &'a Ctx::Address) -> Self {
        Self {
            input_round,
            address,
            proposer: address,
        }
    }

    /// Check if we are the proposer for the round we are at.
    pub fn is_proposer(&self) -> bool {
        self.address == self.proposer
    }
}

/// Check that a proposal has a valid Proof-Of-Lock round
fn is_valid_pol_round<Ctx>(state: &State<Ctx>, pol_round: Round) -> bool
where
    Ctx: Context,
{
    pol_round.is_defined() && pol_round < state.round
}

/// Apply an input to the current state at the current round.
///
/// This function takes the current state and round, and an input,
/// and returns the next state and an optional message for the driver to act on.
///
/// Valid transitions result in at least a change to the state and/or an output.
///
/// Commented numbers refer to line numbers in the spec paper.
pub fn apply<Ctx>(mut state: State<Ctx>, info: &Info<Ctx>, input: Input<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    let this_round = state.round == info.input_round;

    match (state.step, input) {
        //
        // From NewRound.
        //

        // L11/L14
        (Step::Unstarted, Input::NewRound(round)) if info.is_proposer() => {
            // Update the round
            state.round = round;

            debug_trace!(state, Line::L11Proposer);

            // We are the proposer
            propose_valid_or_get_value(state, info.address)
        }

        // L11/L20
        (Step::Unstarted, Input::NewRound(round)) => {
            // Update the round
            state.round = round;

            debug_trace!(state, Line::L11NonProposer);

            // We are not the proposer
            schedule_timeout_propose(state)
        }

        //
        // From Propose. Input must be for current round.
        //

        // L18
        (Step::Propose, Input::ProposeValue(value)) if this_round => {
            debug_assert!(info.is_proposer());

            propose(state, value, info.address)
        }

        // L22 with valid proposal
        (Step::Propose, Input::Proposal(proposal))
            if this_round && proposal.pol_round().is_nil() =>
        {
            debug_trace!(state, Line::L22);

            prevote(state, info.address, &proposal)
        }

        // L22 with invalid proposal
        (Step::Propose, Input::InvalidProposal) if this_round => prevote_nil(state, info.address),

        // L28 with valid proposal
        (Step::Propose, Input::ProposalAndPolkaPrevious(proposal))
            if this_round && is_valid_pol_round(&state, proposal.pol_round()) =>
        {
            debug_trace!(state, Line::L28ValidProposal);
            prevote_previous(state, info.address, &proposal)
        }

        // L28 with invalid proposal
        (Step::Propose, Input::InvalidProposalAndPolkaPrevious(proposal))
            if this_round && is_valid_pol_round(&state, proposal.pol_round()) =>
        {
            debug_trace!(state, Line::L28InvalidProposal);
            debug_trace!(state, Line::L32InvalidValue);

            prevote_nil(state, info.address)
        }

        // L57
        // We are the proposer.
        (Step::Propose, Input::TimeoutPropose) if this_round && info.is_proposer() => {
            debug_trace!(state, Line::L59Proposer);

            prevote_nil(state, info.address)
        }

        // L57
        // We are not the proposer.
        (Step::Propose, Input::TimeoutPropose) if this_round => {
            debug_trace!(state, Line::L59NonProposer);

            prevote_nil(state, info.address)
        }

        //
        // From Prevote. Input must be for current round.
        //

        // L34
        (Step::Prevote, Input::PolkaAny) if this_round => {
            debug_trace!(state, Line::L34);
            debug_trace!(state, Line::L35);

            schedule_timeout_prevote(state)
        }

        // L45
        (Step::Prevote, Input::PolkaNil) if this_round => {
            debug_trace!(state, Line::L45);

            precommit_nil(state, info.address)
        }

        // L36/L37
        // NOTE: Only executed the first time, as the votekeeper will only emit this threshold once.
        (Step::Prevote, Input::ProposalAndPolkaCurrent(proposal)) if this_round => {
            debug_trace!(state, Line::L36ValidProposal);

            precommit(state, info.address, proposal)
        }

        // L61
        (Step::Prevote, Input::TimeoutPrevote) if this_round => {
            debug_trace!(state, Line::L61);

            precommit_nil(state, info.address)
        }

        //
        // From Precommit
        //

        // L36/L42
        // NOTE: Only executed the first time, as the votekeeper will only emit this threshold once.
        (Step::Precommit, Input::ProposalAndPolkaCurrent(proposal)) if this_round => {
            debug_trace!(state, Line::L36ValidProposal);
            set_valid_value(state, &proposal)
        }

        //
        // From Commit. No more state transitions.
        //
        (Step::Commit, _) => Transition::invalid(state),

        //
        // From all (except Commit). Various round guards.
        //

        // L47
        (_, Input::PrecommitAny) if this_round => {
            debug_trace!(state, Line::L48);
            schedule_timeout_precommit(state)
        }

        // L65
        (_, Input::TimeoutPrecommit) if this_round => {
            debug_trace!(state, Line::L67);
            round_skip(state, info.input_round.increment())
        }

        // L55
        (_, Input::SkipRound(round)) if state.round < round => {
            debug_trace!(state, Line::L55);
            round_skip(state, round)
        }

        // L49
        (_, Input::ProposalAndPrecommitValue(proposal)) => {
            let round = state.round;
            debug_trace!(state, Line::L49);
            commit(state, round, proposal)
        }

        // Invalid transition.
        _ => Transition::invalid(state),
    }
}

//---------------------------------------------------------------------
// Propose
//---------------------------------------------------------------------

/// We are the proposer. Propose the valid value if present, otherwise schedule timeout propose
/// and ask for a value.
///
/// Ref: L13-L16, L19
pub fn propose_valid_or_get_value<Ctx>(
    mut state: State<Ctx>,
    address: &Ctx::Address,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    debug_trace!(state, Line::L14);

    match &state.valid {
        Some(round_value) => {
            // L16
            let pol_round = round_value.round;
            let proposal = Output::proposal(
                state.height,
                state.round,
                round_value.value.clone(),
                pol_round,
                address.clone(),
            );
            debug_trace!(state, Line::L16);
            debug_trace!(state, Line::L19);

            Transition::to(state.with_step(Step::Propose)).with_output(proposal)
        }
        None => {
            // L18
            let output = Output::get_value_and_schedule_timeout(
                state.height,
                state.round,
                TimeoutKind::Propose,
            );
            debug_trace!(state, Line::L18);

            Transition::to(state.with_step(Step::Propose)).with_output(output)
        }
    }
}

/// We are the proposer; propose the valid value if it exists,
/// otherwise propose the given value.
///
/// Ref: L13, L17-18
pub fn propose<Ctx>(
    mut state: State<Ctx>,
    value: Ctx::Value,
    address: &Ctx::Address,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    let proposal = Output::proposal(
        state.height,
        state.round,
        value,
        Round::Nil,
        address.clone(),
    );

    debug_trace!(state, Line::L19);
    Transition::to(state.with_step(Step::Propose)).with_output(proposal)
}

//---------------------------------------------------------------------
// Prevote
//---------------------------------------------------------------------

/// Received a complete proposal; prevote the value,
/// unless we are locked on something else.
///
/// Ref: L22 with valid proposal
pub fn prevote<Ctx>(
    mut state: State<Ctx>,
    address: &Ctx::Address,
    proposal: &Ctx::Proposal,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    let vr = proposal.pol_round();
    assert_eq!(vr, Round::Nil);
    let proposed = proposal.value().id();
    let value = match &state.locked {
        Some(locked) if locked.value.id() == proposed => {
            // already locked on value
            debug_trace!(state, Line::L24ValidAndLockedValue);
            NilOrVal::Val(proposed)
        }
        Some(_) => {
            // locked on a different value
            debug_trace!(state, Line::L26ValidAndLockedValue);
            NilOrVal::Nil
        }
        None => {
            // not locked, prevote the value
            debug_trace!(state, Line::L24ValidNoLockedRound);
            NilOrVal::Val(proposed)
        }
    };

    let output = Output::prevote(state.height, state.round, value, address.clone());
    Transition::to(state.with_step(Step::Prevote)).with_output(output)
}

/// Received a proposal for a previously seen value and a polka from a previous round; prevote the value,
/// unless we are locked on a different value at a higher round.
///
/// Ref: L28
pub fn prevote_previous<Ctx>(
    mut state: State<Ctx>,
    address: &Ctx::Address,
    proposal: &Ctx::Proposal,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    let vr = proposal.pol_round();
    assert!(vr >= Round::Some(0));
    assert!(vr < proposal.round());

    let proposed = proposal.value().id();
    let value = match &state.locked {
        Some(locked) if locked.round <= vr => {
            // locked on lower or equal round, maybe on different value
            debug_trace!(state, Line::L30ValidLockedRound);
            NilOrVal::Val(proposed)
        }
        Some(locked) if locked.value.id() == proposed => {
            // already locked same value
            debug_trace!(state, Line::L30ValidLockedValue);
            NilOrVal::Val(proposed)
        }
        Some(_) => {
            // we're locked on a different value in a higher round, prevote nil
            debug_trace!(state, Line::L32InvalidValue);
            NilOrVal::Nil
        }
        None => {
            // not locked, prevote the value
            debug_trace!(state, Line::L30ValidNoLockedRound);
            NilOrVal::Val(proposed)
        }
    };

    let output = Output::prevote(state.height, state.round, value, address.clone());
    Transition::to(state.with_step(Step::Prevote)).with_output(output)
}

/// Received a complete proposal for an empty or invalid value, or timed out; prevote nil.
///
/// Ref: L22/L25, L28/L31, L57
pub fn prevote_nil<Ctx>(state: State<Ctx>, address: &Ctx::Address) -> Transition<Ctx>
where
    Ctx: Context,
{
    let output = Output::prevote(state.height, state.round, NilOrVal::Nil, address.clone());

    Transition::to(state.with_step(Step::Prevote)).with_output(output)
}

// ---------------------------------------------------------------------
// Precommit
// ---------------------------------------------------------------------

/// Received a polka for a value; precommit the value.
///
/// Ref: L36
///
/// NOTE: Only one of this and set_valid_value should be called once in a round
///       How do we enforce this?
pub fn precommit<Ctx>(
    state: State<Ctx>,
    address: &Ctx::Address,
    proposal: Ctx::Proposal,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    if state.step != Step::Prevote {
        return Transition::to(state);
    }

    let value = proposal.value();
    let output = Output::precommit(
        state.height,
        state.round,
        NilOrVal::Val(value.id()),
        address.clone(),
    );

    let next = state
        .set_locked(value.clone())
        .set_valid(value.clone())
        .with_step(Step::Precommit);

    Transition::to(next).with_output(output)
}

/// Received a polka for nil or timed out of prevote; precommit nil.
///
/// Ref: L44, L61
pub fn precommit_nil<Ctx>(state: State<Ctx>, address: &Ctx::Address) -> Transition<Ctx>
where
    Ctx: Context,
{
    let output = Output::precommit(state.height, state.round, NilOrVal::Nil, address.clone());
    Transition::to(state.with_step(Step::Precommit)).with_output(output)
}

// ---------------------------------------------------------------------
// Schedule timeouts
// ---------------------------------------------------------------------

/// We're not the proposer; schedule timeout propose.
///
/// Ref: L11, L20
pub fn schedule_timeout_propose<Ctx>(mut state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    debug_trace!(state, Line::L21ProposeTimeoutScheduled);

    let timeout = Output::schedule_timeout(state.round, TimeoutKind::Propose);
    Transition::to(state.with_step(Step::Propose)).with_output(timeout)
}

/// We received a polka for any; schedule timeout prevote.
///
/// Ref: L34
///
/// NOTE: This should only be called once in a round, per the spec,
///       but it's harmless to schedule more timeouts
pub fn schedule_timeout_prevote<Ctx>(state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    let output = Output::schedule_timeout(state.round, TimeoutKind::Prevote);
    Transition::to(state).with_output(output)
}

/// We received +2/3 precommits for any; schedule timeout precommit.
///
/// Ref: L47
pub fn schedule_timeout_precommit<Ctx>(state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    let output = Output::schedule_timeout(state.round, TimeoutKind::Precommit);
    Transition::to(state).with_output(output)
}

//---------------------------------------------------------------------
// Set the valid value.
//---------------------------------------------------------------------

/// We received a polka for a value after we already precommitted.
/// Set the valid value and current round.
///
/// Ref: L36/L42
///
/// NOTE: only one of this and precommit should be called once in a round
pub fn set_valid_value<Ctx>(state: State<Ctx>, proposal: &Ctx::Proposal) -> Transition<Ctx>
where
    Ctx: Context,
{
    Transition::to(state.set_valid(proposal.value().clone()))
}

//---------------------------------------------------------------------
// New round or height
//---------------------------------------------------------------------

/// We finished a round (timeout precommit) or received +1/3 votes
/// from a higher round. Move to the higher round.
///
/// Ref: L65
pub fn round_skip<Ctx>(state: State<Ctx>, round: Round) -> Transition<Ctx>
where
    Ctx: Context,
{
    let new_state = state.with_round(round).with_step(Step::Unstarted);
    Transition::to(new_state).with_output(Output::NewRound(round))
}

/// We received +2/3 precommits for a value - commit and decide that value!
///
/// Ref: L49
pub fn commit<Ctx>(state: State<Ctx>, round: Round, proposal: Ctx::Proposal) -> Transition<Ctx>
where
    Ctx: Context,
{
    let new_state = state
        .set_decision(proposal.value().clone())
        .with_step(Step::Commit);
    let output = Output::decision(round, proposal.clone());
    Transition::to(new_state).with_output(output)
}

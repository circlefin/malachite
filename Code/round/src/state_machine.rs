use malachite_common::{Context, Proposal, Round, TimeoutStep, Value};

use crate::events::Event;
use crate::message::Message;
use crate::state::{State, Step};
use crate::transition::Transition;

/// Immutable information about the event and our node:
/// - Address of our node
/// - Proposer for the round we are at
/// - Round for which the event is for, can be different than the round we are at
pub struct Info<'a, Ctx>
where
    Ctx: Context,
{
    pub event_round: Round,
    pub address: &'a Ctx::Address,
    pub proposer: &'a Ctx::Address,
}

impl<'a, Ctx> Info<'a, Ctx>
where
    Ctx: Context,
{
    pub fn new(event_round: Round, address: &'a Ctx::Address, proposer: &'a Ctx::Address) -> Self {
        Self {
            event_round,
            address,
            proposer,
        }
    }

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

/// Apply an event to the current state at the current round.
///
/// This function takes the current state and round, and an event,
/// and returns the next state and an optional message for the driver to act on.
///
/// Valid transitions result in at least a change to the state and/or an output message.
///
/// Commented numbers refer to line numbers in the spec paper.
pub fn apply_event<Ctx>(
    mut state: State<Ctx>,
    info: &Info<Ctx>,
    event: Event<Ctx>,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    let this_round = state.round == info.event_round;

    match (state.step, event) {
        // From NewRound. Event must be for current round.

        // We are the proposer
        (Step::NewRound, Event::NewRound) if this_round && info.is_proposer() => {
            propose_valid_or_get_value(state) // L18
        }
        // We are not the proposer
        (Step::NewRound, Event::NewRound) if this_round => schedule_timeout_propose(state), // L11/L20

        // From Propose. Event must be for current round.
        (Step::Propose, Event::ProposeValue(value)) if this_round => {
            debug_assert!(info.is_proposer());
            propose(state, value) // L11/L14
        }

        (Step::Propose, Event::Proposal(proposal))
            if this_round && proposal.pol_round().is_nil() =>
        {
            // L22
            if state
                .locked
                .as_ref()
                .map_or(true, |locked| &locked.value == proposal.value())
            {
                state.proposal = Some(proposal.clone());
                prevote(state, info.address, &proposal)
            } else {
                prevote_nil(state, info.address)
            }
        }

        (Step::Propose, Event::ProposalAndPolkaPrevious(proposal))
            if this_round && is_valid_pol_round(&state, proposal.pol_round()) =>
        {
            // L28
            let Some(locked) = state.locked.as_ref() else {
                return prevote_nil(state, info.address);
            };

            if locked.round <= proposal.pol_round() || &locked.value == proposal.value() {
                prevote(state, info.address, &proposal)
            } else {
                prevote_nil(state, info.address)
            }
        }

        (Step::Propose, Event::ProposalInvalid) if this_round => prevote_nil(state, info.address), // L22/L25, L28/L31

        // We are the proposer.
        (Step::Propose, Event::TimeoutPropose) if this_round && info.is_proposer() => {
            // TOOD: Do we need to do something else here?
            prevote_nil(state, info.address) // L57
        }
        // We are not the proposer.
        (Step::Propose, Event::TimeoutPropose) if this_round => prevote_nil(state, info.address), // L57

        // From Prevote. Event must be for current round.
        (Step::Prevote, Event::PolkaAny) if this_round => schedule_timeout_prevote(state), // L34
        (Step::Prevote, Event::PolkaNil) if this_round => precommit_nil(state, info.address), // L44
        (Step::Prevote, Event::ProposalAndPolkaCurrent(proposal)) if this_round => {
            precommit(state, info.address, proposal) // L36/L37 - NOTE: only once?
        }
        (Step::Prevote, Event::TimeoutPrevote) if this_round => precommit_nil(state, info.address), // L61

        // From Precommit. Event must be for current round.
        (Step::Precommit, Event::ProposalAndPolkaCurrent(proposal)) if this_round => {
            set_valid_value(state, proposal.value().clone()) // L36/L42 - NOTE: only once?
        }

        // From Commit. No more state transitions.
        (Step::Commit, _) => Transition::invalid(state),

        // From all (except Commit). Various round guards.
        (_, Event::PrecommitAny) if this_round => schedule_timeout_precommit(state), // L47
        (_, Event::TimeoutPrecommit) if this_round => {
            round_skip(state, info.event_round.increment())
        } // L65
        (_, Event::SkipRound(round)) if state.round < round => round_skip(state, round), // L55
        (_, Event::ProposalAndPrecommitValue(proposal)) => {
            commit(state, info.event_round, proposal)
        } // L49

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
/// Ref: L15-L18
pub fn propose_valid_or_get_value<Ctx>(state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    match &state.valid {
        Some(round_value) => {
            let pol_round = round_value.round;
            let proposal = Message::proposal(
                state.height.clone(),
                state.round,
                round_value.value.clone(),
                pol_round,
            );
            Transition::to(state.with_step(Step::Propose)).with_message(proposal)
        }
        None => {
            let timeout =
                Message::get_value_and_schedule_timeout(state.round, TimeoutStep::Propose);
            Transition::to(state.with_step(Step::Propose)).with_message(timeout)
        }
    }
}

/// We are the proposer; propose the valid value if it exists,
/// otherwise propose the given value.
///
/// Ref: L11/L14
pub fn propose<Ctx>(state: State<Ctx>, value: Ctx::Value) -> Transition<Ctx>
where
    Ctx: Context,
{
    let proposal = Message::proposal(state.height.clone(), state.round, value, Round::Nil);
    Transition::to(state.with_step(Step::Propose)).with_message(proposal)
}

//---------------------------------------------------------------------
// Prevote
//---------------------------------------------------------------------

/// Received a complete proposal; prevote the value,
/// unless we are locked on something else at a higher round.
///
/// Ref: L22/L28
pub fn prevote<Ctx>(
    state: State<Ctx>,
    address: &Ctx::Address,
    proposal: &Ctx::Proposal,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    let vr = proposal.round();
    let proposed = proposal.value().id();
    let value = match &state.locked {
        Some(locked) if locked.round <= vr => Some(proposed), // unlock and prevote
        Some(locked) if locked.value.id() == proposed => Some(proposed), // already locked on value
        Some(_) => None, // we're locked on a higher round with a different value, prevote nil
        None => Some(proposed), // not locked, prevote the value
    };

    let message = Message::prevote(state.height.clone(), state.round, value, address.clone());
    Transition::to(state.with_step(Step::Prevote)).with_message(message)
}

/// Received a complete proposal for an empty or invalid value, or timed out; prevote nil.
///
/// Ref: L22/L25, L28/L31, L57
pub fn prevote_nil<Ctx>(state: State<Ctx>, address: &Ctx::Address) -> Transition<Ctx>
where
    Ctx: Context,
{
    let message = Message::prevote(state.height.clone(), state.round, None, address.clone());
    Transition::to(state.with_step(Step::Prevote)).with_message(message)
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
    mut state: State<Ctx>,
    address: &Ctx::Address,
    proposal: Ctx::Proposal,
) -> Transition<Ctx>
where
    Ctx: Context,
{
    if state.step != Step::Prevote {
        return Transition::to(state.clone());
    }

    let value = proposal.value();
    let message = Message::precommit(
        state.height.clone(),
        state.round,
        Some(value.id()),
        address.clone(),
    );

    let current_value = match state.proposal {
        Some(ref proposal) => proposal.value().clone(),
        None => {
            state.proposal = Some(proposal.clone());
            proposal.value().clone()
        }
    };

    assert_eq!(current_value.id(), value.id());

    let next = state
        .set_locked(value.clone())
        .set_valid(value.clone())
        .with_step(Step::Precommit);

    Transition::to(next).with_message(message)
}

/// Received a polka for nil or timed out of prevote; precommit nil.
///
/// Ref: L44, L61
pub fn precommit_nil<Ctx>(state: State<Ctx>, address: &Ctx::Address) -> Transition<Ctx>
where
    Ctx: Context,
{
    let message = Message::precommit(state.height.clone(), state.round, None, address.clone());
    Transition::to(state.with_step(Step::Precommit)).with_message(message)
}

// ---------------------------------------------------------------------
// Schedule timeouts
// ---------------------------------------------------------------------

/// We're not the proposer; schedule timeout propose.
///
/// Ref: L11, L20
pub fn schedule_timeout_propose<Ctx>(state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    let timeout = Message::schedule_timeout(state.round, TimeoutStep::Propose);
    Transition::to(state.with_step(Step::Propose)).with_message(timeout)
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
    if state.step == Step::Prevote {
        let message = Message::schedule_timeout(state.round, TimeoutStep::Prevote);
        Transition::to(state).with_message(message)
    } else {
        Transition::to(state)
    }
}

/// We received +2/3 precommits for any; schedule timeout precommit.
///
/// Ref: L47
pub fn schedule_timeout_precommit<Ctx>(state: State<Ctx>) -> Transition<Ctx>
where
    Ctx: Context,
{
    let message = Message::schedule_timeout(state.round, TimeoutStep::Precommit);
    Transition::to(state).with_message(message)
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
pub fn set_valid_value<Ctx>(state: State<Ctx>, value: Ctx::Value) -> Transition<Ctx>
where
    Ctx: Context,
{
    // Check that we're locked on this value
    let Some(locked) = state.locked.as_ref() else {
        // TODO: Add logging
        return Transition::invalid(state);
    };

    if locked.value.id() != value.id() {
        // TODO: Add logging
        return Transition::invalid(state);
    }

    Transition::to(state.clone().set_valid(locked.value.clone()))
}

//---------------------------------------------------------------------
// New round or height
//---------------------------------------------------------------------

/// We finished a round (timeout precommit) or received +1/3 votes
/// from a higher round. Move to the higher round.
///
/// Ref: 65
pub fn round_skip<Ctx>(state: State<Ctx>, round: Round) -> Transition<Ctx>
where
    Ctx: Context,
{
    Transition::to(State::new(state.height.clone(), round)).with_message(Message::NewRound(round))
}

/// We received +2/3 precommits for a value - commit and decide that value!
///
/// Ref: L49
pub fn commit<Ctx>(state: State<Ctx>, round: Round, proposal: Ctx::Proposal) -> Transition<Ctx>
where
    Ctx: Context,
{
    // Check that we're locked on this value
    let Some(locked) = state.locked.as_ref() else {
        // TODO: Add logging
        return Transition::invalid(state);
    };

    if locked.value.id() != proposal.value().id() {
        // TODO: Add logging
        return Transition::invalid(state);
    }

    let message = Message::decision(round, locked.value.clone());
    Transition::to(state.with_step(Step::Commit)).with_message(message)
}

//! The Multiplexer is responsible for multiplexing the input data and returning the appropriate event to the Round State Machine.
//!
//! The table below describes the input to the Multiplexer and the output events to the Round State Machine.
//!
//! The input data is:
//! - The step change from the Round State Machine.
//! - The output events from the Vote Keeper.
//! - Proposals and votes from the Driver.
//!
//!
//! | Step Changed To | Vote Keeper Rhreshold | Proposal        | Multiplexed Input to Round SM   | New Step        | Algo Condition | Output                             |
//! |---------------- | --------------------- | --------------- |---------------------------------| ---------       | -------------- | ---------------------------------- |
//! | new(??)         | -                     | -               | NewRound                        | propose         | L11            | …                                  |
//! | any             | PrecommitValue(v)     | Proposal(v)     | PropAndPrecommitValue           | commit          | L49            | decide(v)                          |
//! | any             | PrecommitAny          | \*              | PrecommitAny                    | any (unchanged) | L47            | sch\_precommit\_timer              |
//! | propose         | none                  | InvalidProposal | InvalidProposal                 | prevote         | L22, L26       | prevote\_nil                       |
//! | propose         | none                  | Proposal        | Proposal                        | prevote         | L22, L24       | prevote(v)                         |
//! | propose         | PolkaPrevious(v, vr)  | InvalidProposal | InvalidProposalAndPolkaPrevious | prevote         | L28, L33       | prevote\_nil                       |
//! | propose         | PolkaPrevious(v, vr)  | Proposal(v,vr)  | ProposalAndPolkaPrevious        | prevote         | L28, L30       | prevote(v)                         |
//! | prevote         | PolkaNil              | \*              | PolkaNil                        | precommit       | L44            | precommit\_nil                     |
//! | prevote         | PolkaValue(v)         | Proposal(v)     | ProposalAndPolkaCurrent         | precommit       | L36, L37       | (set locked and valid)precommit(v) |
//! | prevote         | PolkaAny              | \*              | PolkaAny                        | prevote         | L34            | prevote timer                      |
//! | precommit       | PolkaValue(v)         | Proposal(v)     | ProposalAndPolkaCurrent         | precommit       | L36, L42       | (set valid)                        |

use malachite_common::{Context, Proposal, Round, Validity, Value, ValueId, VoteType};
use malachite_round::input::Input as RoundInput;
use malachite_round::state::Step;
use malachite_vote::keeper::Output as VKOutput;
use malachite_vote::keeper::VoteKeeper;
use malachite_vote::Threshold;

use crate::Driver;

impl<Ctx> Driver<Ctx>
where
    Ctx: Context,
{
    /// Process a received proposal relative to the current state of the round, considering
    /// its validity and performing various checks to determine the appropriate round input action.
    ///
    /// This is needed because, depending on the step we are at when we receive the proposal,
    /// and the amount of votes we received for various values (or nil), we need to feed
    /// different inputs to the round state machine, instead of a plain proposal.
    ///
    /// For example, if we have a proposal for a value, and we have a quorum of precommits
    /// for that value, then we need to feed the round state machine a `ProposalAndPrecommitValue`
    /// input instead of a plain `Proposal` input.
    ///
    /// The method follows these steps:
    ///
    /// 1. Check that there is an ongoing round, otherwise return `None`
    ///
    /// 2. Check that the proposal's height matches the current height, otherwise return `None`.
    ///
    /// 3. If the proposal is invalid, the method follows these steps:
    ///    a. If we are at propose step and the proposal's proof-of-lock (POL) round is `Nil`, return
    ///       `RoundInput::InvalidProposal`.
    ///    b. If we are at propose step and there is a polka for a prior-round proof-of-lock (POL),
    ///       return `RoundInput::InvalidProposalAndPolkaPrevious`.
    ///    c. For other steps or if there is no prior-round POL, return `None`.
    ///
    /// 4. Checks that the proposed value has already not already been decided, after storing the
    ///    proposal, but before further processing.
    ///
    /// 5. If a quorum of precommit votes is met for the proposal's value,
    ///    return `RoundInput::ProposalAndPrecommitValue` including the proposal.
    ///
    /// 6. If the proposal is for a different round than the current one, return `None`.
    ///
    /// 7. If a POL is present for the current round and we are beyond the prevote step,
    ///    return `RoundInput::ProposalAndPolkaCurrent`, including the proposal.
    ///
    /// 8. If we are at the propose step, and a prior round POL exists,
    ///    check if the proposal's valid round is equal to the threshold's valid round,
    ///    and then returns `RoundInput::ProposalAndPolkaPrevious`, including the proposal.
    ///
    /// 9. If none of the above conditions are met, simply wrap the proposal in
    ///    `RoundInput::Proposal` and return it.
    pub fn multiplex_proposal(
        &mut self,
        proposal: Ctx::Proposal,
        validity: Validity,
    ) -> Option<RoundInput<Ctx>> {
        // Check that there is an ongoing round
        if self.round_state.round == Round::Nil {
            return None;
        }

        // Check that the proposal is for the current height
        if self.round_state.height != proposal.height() {
            return None;
        }

        // Store the proposal
        self.proposal = Some(proposal.clone());

        // Determine if there is a polka for the proposal
        let polka_for_pol = self.vote_keeper.is_threshold_met(
            &proposal.pol_round(),
            VoteType::Prevote,
            Threshold::Value(proposal.value().id()),
        );

        // Determine if the polka is for a previous round
        let polka_previous = proposal.pol_round().is_defined()
            && polka_for_pol
            && proposal.pol_round() < self.round_state.round;

        // Handle invalid proposal
        if !validity.is_valid() {
            if self.round_state.step == Step::Propose {
                if proposal.pol_round().is_nil() {
                    // L26
                    return Some(RoundInput::InvalidProposal);
                } else if polka_previous {
                    // L32
                    return Some(RoundInput::InvalidProposalAndPolkaPrevious(proposal));
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        // We have a valid proposal.
        // L49
        if self.vote_keeper.is_threshold_met(
            &proposal.round(),
            VoteType::Precommit,
            Threshold::Value(proposal.value().id()),
        ) && self.round_state.decision.is_none()
        {
            return Some(RoundInput::ProposalAndPrecommitValue(proposal));
        }

        // If the proposal is for a different round, drop the proposal
        if self.round_state.round != proposal.round() {
            return None;
        }

        let polka_for_current = self.vote_keeper.is_threshold_met(
            &proposal.round(),
            VoteType::Prevote,
            Threshold::Value(proposal.value().id()),
        );

        let polka_current = polka_for_current && self.round_state.step >= Step::Prevote;

        // L36
        if polka_current {
            return Some(RoundInput::ProposalAndPolkaCurrent(proposal));
        }

        // L28
        if self.round_state.step == Step::Propose && polka_previous {
            return Some(RoundInput::ProposalAndPolkaPrevious(proposal));
        }

        Some(RoundInput::Proposal(proposal))
    }

    /// After a vote threshold change, check if we have a polka for nil, some value or any,
    /// based on the type of threshold and the current proposal.
    pub fn multiplex_vote_threshold(
        &self,
        new_threshold: VKOutput<ValueId<Ctx>>,
    ) -> RoundInput<Ctx> {
        if let Some(proposal) = &self.proposal {
            match new_threshold {
                VKOutput::PolkaAny => RoundInput::PolkaAny,
                VKOutput::PolkaNil => RoundInput::PolkaNil,
                VKOutput::PolkaValue(v) => {
                    if v == proposal.value().id() {
                        RoundInput::ProposalAndPolkaCurrent(proposal.clone())
                    } else {
                        RoundInput::PolkaAny
                    }
                }
                VKOutput::PrecommitAny => RoundInput::PrecommitAny,
                VKOutput::PrecommitValue(v) => {
                    if v == proposal.value().id() {
                        RoundInput::ProposalAndPrecommitValue(proposal.clone())
                    } else {
                        RoundInput::PrecommitAny
                    }
                }
                VKOutput::SkipRound(r) => RoundInput::SkipRound(r),
            }
        } else {
            match new_threshold {
                VKOutput::PolkaAny => RoundInput::PolkaAny,
                VKOutput::PolkaNil => RoundInput::PolkaNil,
                VKOutput::PolkaValue(_) => RoundInput::PolkaAny,
                VKOutput::PrecommitAny => RoundInput::PrecommitAny,
                VKOutput::PrecommitValue(_) => RoundInput::PrecommitAny,
                VKOutput::SkipRound(r) => RoundInput::SkipRound(r),
            }
        }
    }

    /// After a step change, see if there is an input to be
    /// sent implicitly to the round state machine.
    pub fn multiplex_step_change(
        &self,
        pending_step: Step,
        round: Round,
    ) -> Option<RoundInput<Ctx>> {
        match pending_step {
            Step::Unstarted => None,

            Step::Propose => None,

            // After a step change to prevote, check if we have a polka for nil, some value or any,
            // and return the corresponding input for the round state machine.
            Step::Prevote => {
                if has_polka_nil(&self.vote_keeper, round) {
                    Some(RoundInput::PolkaNil)
                } else if let Some(proposal) =
                    has_polka_value(&self.vote_keeper, round, self.proposal.as_ref())
                {
                    Some(RoundInput::ProposalAndPolkaCurrent(proposal.clone()))
                } else if has_polka_any(&self.vote_keeper, round) {
                    Some(RoundInput::PolkaAny)
                } else {
                    None
                }
            }

            Step::Precommit => None,

            Step::Commit => None,
        }
    }
}

/// Check if we have a polka for nil
fn has_polka_nil<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Nil)
}

/// Check if we have a polka for a value
fn has_polka_value<'p, Ctx>(
    votekeeper: &VoteKeeper<Ctx>,
    round: Round,
    proposal: Option<&'p Ctx::Proposal>,
) -> Option<&'p Ctx::Proposal>
where
    Ctx: Context,
{
    let proposal = proposal?;

    votekeeper
        .is_threshold_met(
            &round,
            VoteType::Prevote,
            Threshold::Value(proposal.value().id()),
        )
        .then_some(proposal)
}

/// Check if we have a polka for any
fn has_polka_any<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Any)
}

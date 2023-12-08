use malachite_common::ValueId;
use malachite_common::{Context, Proposal, Round, Value, VoteType};
use malachite_round::input::Input as RoundInput;
use malachite_round::state::Step;
use malachite_vote::keeper::Output as VoteKeeperOutput;
use malachite_vote::keeper::VoteKeeper;
use malachite_vote::Threshold;

use crate::{Driver, Validity};

impl<Ctx> Driver<Ctx>
where
    Ctx: Context,
{
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

        let polka_for_pol = self.vote_keeper.is_threshold_met(
            &proposal.pol_round(),
            VoteType::Prevote,
            Threshold::Value(proposal.value().id()),
        );

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
                    return Some(RoundInput::InvalidProposalAndPolkaPrevious(
                        proposal.clone(),
                    ));
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        // We have a valid proposal.
        // L49
        // TODO - check if not already decided
        if self.vote_keeper.is_threshold_met(
            &proposal.round(),
            VoteType::Precommit,
            Threshold::Value(proposal.value().id()),
        ) {
            return Some(RoundInput::ProposalAndPrecommitValue(proposal.clone()));
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
            // TODO: Check proposal vr is equal to threshold vr
            return Some(RoundInput::ProposalAndPolkaPrevious(proposal));
        }

        Some(RoundInput::Proposal(proposal))
    }

    pub fn multiplex_vote_threshold(
        &self,
        new_threshold: VoteKeeperOutput<ValueId<Ctx>>,
    ) -> Option<RoundInput<Ctx>> {
        if let Some(proposal) = &self.proposal {
            match new_threshold {
                VoteKeeperOutput::PolkaAny => Some(RoundInput::PolkaAny),
                VoteKeeperOutput::PolkaNil => Some(RoundInput::PolkaNil),
                VoteKeeperOutput::PolkaValue(v) => {
                    if v == proposal.value().id() {
                        Some(RoundInput::ProposalAndPolkaCurrent(proposal.clone()))
                    } else {
                        Some(RoundInput::PolkaAny)
                    }
                }
                VoteKeeperOutput::PrecommitAny => Some(RoundInput::PrecommitAny),
                VoteKeeperOutput::PrecommitValue(v) => {
                    if v == proposal.value().id() {
                        Some(RoundInput::ProposalAndPrecommitValue(proposal.clone()))
                    } else {
                        Some(RoundInput::PrecommitAny)
                    }
                }
                VoteKeeperOutput::SkipRound(r) => Some(RoundInput::SkipRound(r)),
            }
        } else {
            match new_threshold {
                VoteKeeperOutput::PolkaAny => Some(RoundInput::PolkaAny),
                VoteKeeperOutput::PolkaNil => Some(RoundInput::PolkaNil),
                VoteKeeperOutput::PolkaValue(_) => Some(RoundInput::PolkaAny),
                VoteKeeperOutput::PrecommitAny => Some(RoundInput::PrecommitAny),
                VoteKeeperOutput::PrecommitValue(_) => Some(RoundInput::PrecommitAny),
                VoteKeeperOutput::SkipRound(r) => Some(RoundInput::SkipRound(r)),
            }
        }
    }

    pub fn multiplex_step_change(
        &self,
        pending_step: Step,
        round: Round,
    ) -> Option<RoundInput<Ctx>> {
        match pending_step {
            Step::NewRound => None, // Some(RoundInput::NewRound),

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

            Step::Propose => None,
            Step::Precommit => None,
            Step::Commit => None,
        }
    }
}

fn has_polka_nil<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Nil)
}

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

fn has_polka_any<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Any)
}

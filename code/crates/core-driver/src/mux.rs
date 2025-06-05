//! The Multiplexer is responsible for multiplexing the input data and returning the appropriate event to the Round State Machine.
//!
//! The table below describes the input to the Multiplexer and the output events to the Round State Machine.
//!
//! The input data is:
//! - Proposals from the Driver.
//! - The output events from the Vote Keeper.
//! - The step change from the Round State Machine.
//!
//! The table below shows the result of multiplexing an input, result that is sent as input to the round state machine, expected effects the tendermint algorithm condition.
//! Looking at the first entry as an example:
//! - If a proposal is received and a quorum of precommits exists, then the `PropAndPrecommitValue` input (L49) is sent to the round state machine.
//!   The round state machine will move to `commit` step, return the `decide(v)` to the driver.
//! - If a vote is received and as a result a quorum of precommits is reached, and if a valid proposal is present, then the same as above happens
//!
//!
//! | Step            | Vote Keeper Threshold | Proposal        | Multiplexed Input to Round SM   | New Step        | Algo Clause    | Output                             |
//! |---------------- | --------------------- | --------------- |---------------------------------| ---------       | -------------- | ---------------------------------- |
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

use alloc::vec::Vec;

use malachitebft_core_state_machine::input::Input as RoundInput;
use malachitebft_core_state_machine::state::Step;
use malachitebft_core_types::{CommitCertificate, PolkaCertificate, SignedProposal};
use malachitebft_core_types::{Context, Proposal, Round, Validity, Value, ValueId, VoteType};
use malachitebft_core_votekeeper::keeper::Output as VKOutput;
use malachitebft_core_votekeeper::keeper::VoteKeeper;
use malachitebft_core_votekeeper::Threshold;

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
    /// 2. If the proposal is invalid, the method follows these steps:
    ///    a. If we are at propose step and the proposal's proof-of-lock (POL) round is `Nil`, return
    ///       `RoundInput::InvalidProposal`.
    ///    b. If we are at propose step and there is a polka for a prior-round proof-of-lock (POL),
    ///       return `RoundInput::InvalidProposalAndPolkaPrevious`.
    ///    c. For other steps or if there is no prior-round POL, return `None`.
    ///
    /// 3. If a quorum of precommit votes is met for the proposal's value,
    ///    return `RoundInput::ProposalAndPrecommitValue` including the proposal.
    ///
    /// 4. If the proposal is for a different round than the current one, return `None`.
    ///
    /// 5. If a polka is present for the current round and we are beyond the prevote step,
    ///    return `RoundInput::ProposalAndPolkaCurrent`, including the proposal.
    ///
    /// 6. If we are at the propose step, and a polka exists for a the propopsal's POL round,
    ///    return `RoundInput::ProposalAndPolkaPrevious`, including the proposal.
    ///
    /// 7. If none of the above conditions are met, simply wrap the proposal in
    ///    `RoundInput::Proposal` and return it.
    pub(crate) fn multiplex_proposal(
        &mut self,
        proposal: Ctx::Proposal,
        validity: Validity,
    ) -> Option<RoundInput<Ctx>> {
        // Should only receive proposals for our height.
        assert_eq!(self.height(), proposal.height());

        // Check that there is an ongoing round
        if self.round_state.round == Round::Nil {
            return None;
        }

        // Find the polka certificate for the pol_round
        let polka_certificate_for_previous = self
            .polka_certificates
            .iter()
            .find(|cert| cert.round == proposal.pol_round());

        // Determine if there is a polka for a previous round, either from the vote keeper or from the polka certificate
        let polka_previous = proposal.pol_round().is_defined()
            && proposal.pol_round() < self.round_state.round
            && (polka_certificate_for_previous.is_some()
                || self.vote_keeper.is_threshold_met(
                    &proposal.pol_round(),
                    VoteType::Prevote,
                    Threshold::Value(proposal.value().id()),
                ));

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

        // We have a valid proposal. Check if there is already a certificate for it.
        // L49
        if self.round_state.decision.is_none()
            && self
                .commit_certificate(proposal.round(), proposal.value().id())
                .is_some()
        {
            return Some(RoundInput::ProposalAndPrecommitValue(proposal));
        }

        if self.vote_keeper.is_threshold_met(
            &proposal.round(),
            VoteType::Precommit,
            Threshold::Value(proposal.value().id()),
        ) && self.round_state.decision.is_none()
        {
            return Some(RoundInput::ProposalAndPrecommitValue(proposal));
        }

        // If the proposal is for a different round, return.
        // This check must be after the L49 check above because a commit quorum from any round
        // should result in a decision.
        if self.round_state.round != proposal.round() {
            return None;
        }

        // Find the polka certificate for the current round
        let polka_certificate_for_current = self
            .polka_certificates
            .iter()
            .find(|cert| cert.round == proposal.round());

        // Determine if there is a polka for the current round, either from the vote keeper or from the polka certificate
        let polka_for_current = polka_certificate_for_current.is_some()
            || self.vote_keeper.is_threshold_met(
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

        if proposal.pol_round().is_nil() {
            // L22
            return Some(RoundInput::Proposal(proposal));
        }

        // We have `vr >= 0` without a  matching polka from round `vr`,
        // so we do not do anything and wait either:
        // - For more votes to arrive and form a polka
        // - For the Propose timeout to expire, prevote nil and move to prevote
        None
    }

    pub(crate) fn store_and_multiplex_proposal(
        &mut self,
        signed_proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Option<RoundInput<Ctx>> {
        // Should only receive proposals for our height.
        assert_eq!(self.height(), signed_proposal.height());

        let proposal = signed_proposal.message.clone();

        // Store the proposal and its validity
        self.proposal_keeper
            .store_proposal(signed_proposal, validity);

        self.multiplex_proposal(proposal, validity)
    }

    pub(crate) fn store_and_multiplex_commit_certificate(
        &mut self,
        certificate: CommitCertificate<Ctx>,
    ) -> Option<RoundInput<Ctx>> {
        // Should only receive proposals for our height.
        assert_eq!(self.height(), certificate.height);

        let certificate_round = certificate.round;
        let certificate_value_id = certificate.value_id.clone();

        // Store the certificate
        self.commit_certificates.push(certificate);

        if let Some((signed_proposal, validity)) =
            self.proposal_and_validity_for_round_and_value(certificate_round, certificate_value_id)
        {
            if validity.is_valid() {
                return Some(RoundInput::ProposalAndPrecommitValue(
                    signed_proposal.message.clone(),
                ));
            }
        }

        None
    }

    /// Store the polka certificate and multiplex the proposal.
    /// Returns the round and the input to the round state machine.
    /// The round is:
    /// - the round of the Proposal with a pol_round matching the certificate's round (L28), or
    /// - the matching proposal is at the same round (L36), or
    /// - the round of the certificate if there is no matching proposal (L44)
    pub(crate) fn store_and_multiplex_polka_certificate(
        &mut self,
        certificate: PolkaCertificate<Ctx>,
    ) -> Option<(Round, RoundInput<Ctx>)> {
        // Should only receive proposals for our height.
        assert_eq!(self.height(), certificate.height);

        let certificate_round = certificate.round;
        let certificate_value_id = certificate.value_id.clone();

        // Only add if an identical certificate isn't already present
        if !self.polka_certificates.iter().any(|existing| {
            existing.round == certificate.round && existing.value_id == certificate.value_id
        }) {
            self.polka_certificates.push(certificate);
        }

        let Some((signed_proposal, validity)) = self
            .proposal_and_validity_for_round_and_value(self.round(), certificate_value_id.clone())
        else {
            return Some((certificate_round, RoundInput::PolkaAny));
        };

        let proposal = &signed_proposal.message;

        if certificate_value_id == proposal.value().id() {
            if validity.is_valid() {
                if proposal.pol_round() == certificate_round {
                    Some((
                        proposal.round(),
                        RoundInput::ProposalAndPolkaPrevious(proposal.clone()),
                    ))
                } else if proposal.round() == certificate_round {
                    Some((
                        proposal.round(),
                        RoundInput::ProposalAndPolkaCurrent(proposal.clone()),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            Some((certificate_round, RoundInput::PolkaAny))
        }
    }

    /// After a vote threshold change for a given round, check if we have a polka for nil, some value or any,
    /// based on the type of threshold and the current proposal.
    pub(crate) fn multiplex_vote_threshold(
        &mut self,
        new_threshold: VKOutput<ValueId<Ctx>>,
        threshold_round: Round,
    ) -> (Round, RoundInput<Ctx>) {
        match new_threshold {
            VKOutput::PolkaAny => (threshold_round, RoundInput::PolkaAny),
            VKOutput::PolkaNil => (threshold_round, RoundInput::PolkaNil),
            VKOutput::PrecommitAny => (threshold_round, RoundInput::PrecommitAny),
            VKOutput::SkipRound(r) => (threshold_round, RoundInput::SkipRound(r)),
            VKOutput::PrecommitValue(v) => {
                if let Some((proposal, validity)) =
                    self.proposal_and_validity_for_round_and_value(threshold_round, v)
                {
                    if validity.is_valid() {
                        (
                            threshold_round,
                            RoundInput::ProposalAndPrecommitValue(proposal.message.clone()),
                        )
                    } else {
                        (threshold_round, RoundInput::PrecommitAny)
                    }
                } else {
                    (threshold_round, RoundInput::PrecommitAny)
                }
            }
            VKOutput::PolkaValue(v) => {
                if let Some((proposal, validity)) =
                    self.proposal_and_validity_for_round_and_value(self.round(), v)
                {
                    let proposal = &proposal.message;
                    // We have a proposal for the same value as the threshold.
                    // validity  proposal(v, roundp, pol_round)      threshold(v, threshold_round) Output Line
                    // =================================================================================
                    // invalid   (v, roundp, pol_round)              (v, pol_round)             ProposalAndPolkaPrevious L32
                    // valid     (v, roundp, pol_round)              (v, pol_round)             InvalidProposalAndPolkaPrevious L30
                    //
                    // valid     (v, roundp, pol_round)              (v, roundp)                ProposalAndPolkaCurrent L36
                    // valid     (v, roundp, nil)                    (v, roundp)                ProposalAndPolkaCurrent L36
                    //
                    // *         *                                   (v, threshold_round)       PolkaAny L34
                    let proposal_round = proposal.round();
                    let pol_round = proposal.pol_round();
                    let pol_round_match = pol_round == threshold_round;
                    let round_match = proposal_round == threshold_round;

                    match pol_round {
                        // L32 - state machine will vote nil
                        Round::Some(_) if !validity.is_valid() && pol_round_match => (
                            proposal_round,
                            RoundInput::InvalidProposalAndPolkaPrevious(proposal.clone()),
                        ),
                        // L30
                        Round::Some(_) if validity.is_valid() && pol_round_match => (
                            proposal_round,
                            RoundInput::ProposalAndPolkaPrevious(proposal.clone()),
                        ),
                        // L36 with pol_round != *
                        _ if round_match && validity.is_valid() => (
                            threshold_round,
                            RoundInput::ProposalAndPolkaCurrent(proposal.clone()),
                        ),
                        _ => (threshold_round, RoundInput::PolkaAny),
                    }
                } else {
                    // L34
                    (threshold_round, RoundInput::PolkaAny)
                }
            }
        }
    }

    /// After a step change, check for inputs to be sent to the round state machine.
    pub(crate) fn multiplex_step_change(&mut self, round: Round) -> Vec<(Round, RoundInput<Ctx>)> {
        let mut result = Vec::new();

        let proposals = self.proposals_and_validities_for_round(round).to_vec();

        for (signed_proposal, validity) in proposals {
            let proposal = &signed_proposal.message;
            let step = self.round_state().step;

            match step {
                Step::Propose => {
                    if let Some(input) = self.multiplex_proposal(proposal.clone(), validity) {
                        result.push((self.round(), input))
                    }
                }

                Step::Prevote if has_polka_value(&self.vote_keeper, round, proposal) => result
                    .push(self.multiplex_vote_threshold(
                        VKOutput::PolkaValue(proposal.value().id()),
                        round,
                    )),

                _ => {}
            }
        }

        if let Some(threshold) = find_non_value_threshold(&self.vote_keeper, round) {
            result.push(self.multiplex_vote_threshold(threshold, round))
        }

        result
    }
}

fn find_non_value_threshold<Ctx>(
    votekeeper: &VoteKeeper<Ctx>,
    round: Round,
) -> Option<VKOutput<ValueId<Ctx>>>
where
    Ctx: Context,
{
    if has_precommit_any(votekeeper, round) {
        Some(VKOutput::PrecommitAny)
    } else if has_polka_nil(votekeeper, round) {
        Some(VKOutput::PolkaNil)
    } else if has_polka_any(votekeeper, round) {
        Some(VKOutput::PolkaAny)
    } else {
        None
    }
}

/// Check if we have a polka for a value
fn has_polka_value<Ctx>(
    votekeeper: &VoteKeeper<Ctx>,
    round: Round,
    proposal: &Ctx::Proposal,
) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(
        &round,
        VoteType::Prevote,
        Threshold::Value(proposal.value().id()),
    )
}

/// Check if we have a polka for nil
fn has_polka_nil<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Nil)
}

/// Check if we have a polka for any
fn has_polka_any<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Any)
}

/// Check if we have a quorum of precommits for any
fn has_precommit_any<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Precommit, Threshold::Any)
}

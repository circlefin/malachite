use malachitebft_core_driver::Input as DriverInput;
use malachitebft_core_driver::Output as DriverOutput;

use crate::handle::decide::decide;
use crate::handle::on_proposal;
use crate::handle::signature::sign_proposal;
use crate::handle::signature::sign_vote;
use crate::handle::vote::on_vote;
use crate::params::HIDDEN_LOCK_ROUND;
use crate::prelude::*;
use crate::types::{
    LivenessMsg, {LocallyProposedValue, SignedConsensusMsg},
};
use crate::util::pretty::PrettyVal;
use crate::Role;

use super::propose::on_propose;

#[async_recursion]
pub async fn apply_driver_input<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    input: DriverInput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match &input {
        DriverInput::NewRound(height, round, proposer) => {
            #[cfg(feature = "metrics")]
            metrics.round.set(round.as_i64());

            // Publishing the round certificate upon entering round > 0
            // is part of the new round synchronization mechanism, which
            // ensures all validators advance through rounds even in the
            // presence of asynchrony or Byzantine behavior. Moreover,
            // it guarantees that after GST, all correct replicas will receive
            // the round certificate and enter the same round within bounded time.
            if round > &Round::new(0) {
                if let Some(cert) = state.driver.round_certificate() {
                    if cert.enter_round == *round {
                        info!(
                            %cert.certificate.height,
                            %cert.enter_round,
                            number_of_votes = cert.certificate.round_signatures.len(),
                            "Sending round certificate"
                        );
                        perform!(
                            co,
                            Effect::PublishLivenessMsg(
                                LivenessMsg::SkipRoundCertificate(cert.certificate.clone()),
                                Default::default()
                            )
                        );
                    }
                }
            }

            let role = if state.address() == proposer {
                Role::Proposer
            } else if state.is_validator() {
                Role::Validator
            } else {
                Role::None
            };

            info!(%height, %round, %proposer, ?role, "Starting new round");

            state.last_signed_prevote = None;
            state.last_signed_precommit = None;

            perform!(co, Effect::CancelAllTimeouts(Default::default()));
            perform!(
                co,
                Effect::StartRound(*height, *round, proposer.clone(), role, Default::default())
            );

            #[cfg(feature = "metrics")]
            metrics.rebroadcast_timeouts.inc();

            // Schedule rebroadcast timer
            let timeout = Timeout::rebroadcast(*round);
            perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));
        }

        DriverInput::ProposeValue(round, _) => {
            perform!(
                co,
                Effect::CancelTimeout(Timeout::propose(*round), Default::default())
            );
        }

        DriverInput::Proposal(proposal, _validity) => {
            if proposal.height() != state.driver.height() {
                warn!(
                    "Ignoring proposal for height {}, current height: {}",
                    proposal.height(),
                    state.driver.height()
                );

                return Ok(());
            }
        }

        DriverInput::Vote(vote) => {
            if vote.height() != state.driver.height() {
                warn!(
                    "Ignoring vote for height {}, current height: {}",
                    vote.height(),
                    state.driver.height()
                );

                return Ok(());
            }
        }

        DriverInput::CommitCertificate(certificate) => {
            if certificate.height != state.driver.height() {
                warn!(
                    "Ignoring commit certificate for height {}, current height: {}",
                    certificate.height,
                    state.driver.height()
                );

                return Ok(());
            }
        }

        DriverInput::PolkaCertificate(certificate) => {
            if certificate.height != state.driver.height() {
                warn!(
                    "Ignoring polka certificate for height {}, current height: {}",
                    certificate.height,
                    state.driver.height()
                );

                return Ok(());
            }
        }

        DriverInput::TimeoutElapsed(_) => (),
    }

    // Record the step we were in
    let prev_step = state.driver.step();

    let outputs = state
        .driver
        .process(input)
        .map_err(|e| Error::DriverProcess(e))?;

    // Record the step we are now at
    let new_step = state.driver.step();

    // If the step has changed, update the metrics
    if prev_step != new_step {
        debug!(step.previous = ?prev_step, step.new = ?new_step, "Transitioned to new step");

        if let Some(valid) = &state.driver.valid_value() {
            if state.driver.step_is_propose() {
                info!(
                    round = %valid.round,
                    "Entering Propose step with a valid value"
                );
            }
        }

        #[cfg(feature = "metrics")]
        {
            metrics.step_end(prev_step);
            metrics.step_start(new_step);
        }
    }

    if prev_step != new_step && state.driver.step_is_prevote() {
        // Cancel the Propose timeout since we have moved from Propose to Prevote
        perform!(
            co,
            Effect::CancelTimeout(Timeout::propose(state.driver.round()), Default::default())
        );
    }

    process_driver_outputs(co, state, metrics, outputs).await?;

    Ok(())
}

async fn process_driver_outputs<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    outputs: Vec<DriverOutput<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    for output in outputs {
        process_driver_output(co, state, metrics, output).await?;
    }

    Ok(())
}

async fn process_driver_output<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    output: DriverOutput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match output {
        DriverOutput::NewRound(height, round) => {
            let proposer = state.get_proposer(height, round);
            apply_driver_input(
                co,
                state,
                metrics,
                DriverInput::NewRound(height, round, proposer.clone()),
            )
            .await
        }

        DriverOutput::Propose(proposal) => {
            info!(
                id = %proposal.value().id(),
                round = %proposal.round(),
                "Proposing value"
            );

            // Only sign and publish if we're in the validator set
            if state.is_validator() {
                let signed_proposal = sign_proposal(co, proposal.clone()).await?;

                if signed_proposal.pol_round().is_defined() {
                    perform!(
                        co,
                        Effect::RestreamProposal(
                            signed_proposal.height(),
                            signed_proposal.round(),
                            signed_proposal.pol_round(),
                            signed_proposal.validator_address().clone(),
                            signed_proposal.value().id(),
                            Default::default()
                        )
                    );
                }

                on_proposal(co, state, metrics, signed_proposal.clone()).await?;

                // Proposal messages should not be broadcasted if they are implicit,
                // instead they should be inferred from the block parts.
                if state.params.value_payload.include_proposal() {
                    perform!(
                        co,
                        Effect::PublishConsensusMsg(
                            SignedConsensusMsg::Proposal(signed_proposal),
                            Default::default()
                        )
                    );
                };

                // Publishing the polka certificate of the re-proposed value
                // ensures all validators receive it, which is necessary for
                // them to accept the re-proposed value.
                if proposal.pol_round().is_defined() {
                    // Broadcast the polka certificate at pol_round
                    let Some(polka_certificate) =
                        state.polka_certificate_at_round(proposal.pol_round())
                    else {
                        panic!(
                            "Missing polka certificate for pol_round {}",
                            proposal.pol_round()
                        );
                    };
                    perform!(
                        co,
                        Effect::PublishLivenessMsg(
                            LivenessMsg::PolkaCertificate(polka_certificate),
                            Default::default()
                        )
                    );
                }
            }

            Ok(())
        }

        DriverOutput::Vote(vote) => {
            // Upon locking, in addition to publishing a Precommit message,
            // a validator must request the application to restream the proposal,
            // publish the proposal message, and publish the polka certificate.
            // In other words, it must ensure that all validators receive the same events
            // that led it to lock a value. Together with the timeout mechanisms,
            // this guarantees that after GST, all correct validators will update
            // their validValue and validRound to these values in this round.
            // As a result, Malachite ensures liveness, because all validators
            // will be aware of the most recently locked value, and whichever validator
            // becomes the leader in one of the following rounds will propose a value
            // that all correct validators can accept.
            // Importantly, this mechanism does not need to be enabled from round 0,
            // as it is expensive; it can be activated from any round as a last-resort
            // backup to guarantee liveness.
            if let (VoteType::Precommit, NilOrVal::Val(value_id)) = (vote.vote_type(), vote.value())
            {
                // Prune all votes and certificates for the previous rounds as we know we are not going to use them anymore.
                state.driver.prune_votes_and_certificates(vote.round());

                if state.driver.round() >= HIDDEN_LOCK_ROUND {
                    if let Some((signed_proposal, Validity::Valid)) = state
                        .driver
                        .proposal_and_validity_for_round_and_value(vote.round(), value_id.clone())
                    {
                        perform!(
                            co,
                            Effect::RestreamProposal(
                                signed_proposal.height(),
                                signed_proposal.round(),
                                signed_proposal.pol_round(),
                                signed_proposal.validator_address().clone(),
                                signed_proposal.value().id(),
                                Default::default()
                            )
                        );

                        if state.params.value_payload.include_proposal() {
                            perform!(
                                co,
                                Effect::PublishConsensusMsg(
                                    SignedConsensusMsg::Proposal(signed_proposal.clone()),
                                    Default::default()
                                )
                            );
                        }

                        let Some(polka_certificate) =
                            state.polka_certificate_at_round(vote.round())
                        else {
                            panic!(
                                "Missing polka certificate for Precommit({:?}) at round {}",
                                vote.value(),
                                vote.round()
                            );
                        };
                        perform!(
                            co,
                            Effect::PublishLivenessMsg(
                                LivenessMsg::PolkaCertificate(polka_certificate),
                                Default::default()
                            )
                        );
                    }
                }
            }

            if state.is_validator() {
                info!(
                    vote_type = ?vote.vote_type(),
                    value = %PrettyVal(vote.value().as_ref()),
                    round = %vote.round(),
                    "Voting",
                );

                let extended_vote = extend_vote(co, vote).await?;
                let signed_vote = sign_vote(co, extended_vote).await?;

                on_vote(co, state, metrics, signed_vote.clone()).await?;

                perform!(
                    co,
                    Effect::PublishConsensusMsg(
                        SignedConsensusMsg::Vote(signed_vote.clone()),
                        Default::default()
                    )
                );

                state.set_last_vote(signed_vote);

                // Schedule rebroadcast timer
                let timeout = Timeout::rebroadcast(state.driver.round());
                perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));
            }

            Ok(())
        }

        DriverOutput::Decide(consensus_round, proposal) => {
            info!(
                round = %consensus_round,
                height = %proposal.height(),
                value = %proposal.value().id(),
                "Decided",
            );

            decide(co, state, metrics).await?;

            Ok(())
        }

        DriverOutput::ScheduleTimeout(timeout) => {
            info!(round = %timeout.round, step = ?timeout.kind, "Scheduling timeout");

            perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));

            Ok(())
        }

        DriverOutput::GetValue(height, round, timeout) => {
            if let Some(full_proposal) =
                state.full_proposal_at_round_and_proposer(&height, round, state.address())
            {
                info!(%height, %round, "Using already existing value");

                let local_value = LocallyProposedValue {
                    height: full_proposal.proposal.height(),
                    round: full_proposal.proposal.round(),
                    value: full_proposal.builder_value.clone(),
                };

                on_propose(co, state, metrics, local_value).await?;
            } else {
                info!(%height, %round, "Requesting value from application");

                perform!(
                    co,
                    Effect::GetValue(height, round, timeout, Default::default())
                );
            }

            Ok(())
        }
    }
}

async fn extend_vote<Ctx: Context>(co: &Co<Ctx>, vote: Ctx::Vote) -> Result<Ctx::Vote, Error<Ctx>> {
    let VoteType::Precommit = vote.vote_type() else {
        return Ok(vote);
    };

    let NilOrVal::Val(value_id) = vote.value().as_ref().cloned() else {
        return Ok(vote);
    };

    let extension = perform!(
        co,


        Effect::ExtendVote(vote.height(), vote.round(), value_id, Default::default()),
        Resume::VoteExtension(extension) => extension);

    if let Some(extension) = extension {
        Ok(vote.extend(extension))
    } else {
        Ok(vote)
    }
}

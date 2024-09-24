use async_recursion::async_recursion;
use tracing::{debug, error, info, warn};

use malachite_common::*;
use malachite_driver::Input as DriverInput;
use malachite_driver::Output as DriverOutput;
use malachite_metrics::Metrics;

use crate::effect::{Effect, Resume};
use crate::error::Error;
use crate::gen::Co;
use crate::msg::Msg;
use crate::perform;
use crate::state::State;
use crate::types::{GossipMsg, ProposedValue};
use crate::util::pretty::{PrettyProposal, PrettyVal, PrettyVote};
use crate::ConsensusMsg;

pub async fn handle<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    msg: Msg<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    handle_msg(&co, state, metrics, msg).await
}

#[async_recursion]
async fn handle_msg<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    msg: Msg<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match msg {
        Msg::StartHeight(height) => reset_and_start_height(co, state, metrics, height).await,
        Msg::Vote(vote) => on_vote(co, state, metrics, vote).await,
        Msg::Proposal(proposal) => on_proposal(co, state, metrics, proposal).await,
        Msg::ProposeValue(height, round, value) => {
            propose_value(co, state, metrics, height, round, value).await
        }
        Msg::TimeoutElapsed(timeout) => on_timeout_elapsed(co, state, metrics, timeout).await,
        Msg::ReceivedProposedValue(block) => {
            on_received_proposed_value(co, state, metrics, block).await
        }
    }
}

async fn reset_and_start_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    perform!(co, Effect::CancelAllTimeouts);
    perform!(co, Effect::ResetTimeouts);

    metrics.step_end(state.driver.step());

    let validator_set = perform!(co, Effect::GetValidatorSet(height),
        Resume::ValidatorSet(vs_height, validator_set) => {
            if vs_height == height {
                Ok(validator_set)
            } else {
                Err(Error::UnexpectedResume(
                    Resume::ValidatorSet(vs_height, validator_set),
                    "ValidatorSet for the current height"
                ))
            }
        }
    )?;

    state.driver.move_to_height(height, validator_set);

    debug_assert_eq!(state.driver.height(), height);
    debug_assert_eq!(state.driver.round(), Round::Nil);

    start_height(co, state, metrics, height).await
}

async fn start_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let round = Round::new(0);
    info!(%height, "Starting new height");

    let proposer = state.get_proposer(height, round).cloned()?;

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::NewRound(height, round, proposer.clone()),
    )
    .await?;

    metrics.block_start();
    metrics.height.set(height.as_u64() as i64);
    metrics.round.set(round.as_i64());

    perform!(co, Effect::StartRound(height, round, proposer));

    replay_pending_msgs(co, state, metrics).await?;

    Ok(())
}

async fn replay_pending_msgs<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let pending_msgs = std::mem::take(&mut state.msg_queue);
    debug!("Replaying {} messages", pending_msgs.len());

    for pending_msg in pending_msgs {
        handle_msg(co, state, metrics, pending_msg).await?;
    }

    Ok(())
}

#[async_recursion]
async fn apply_driver_input<Ctx>(
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
            metrics.round.set(round.as_i64());

            info!(%height, %round, %proposer, "Starting new round");
            perform!(co, Effect::CancelAllTimeouts);
            perform!(co, Effect::StartRound(*height, *round, proposer.clone()));
        }

        DriverInput::ProposeValue(round, _) => {
            perform!(co, Effect::CancelTimeout(Timeout::propose(*round)));
        }

        DriverInput::Proposal(proposal, _) => {
            if proposal.height() != state.driver.height() {
                warn!(
                    "Ignoring proposal for height {}, current height: {}",
                    proposal.height(),
                    state.driver.height()
                );

                return Ok(());
            }

            perform!(
                co,
                Effect::CancelTimeout(Timeout::propose(proposal.round()))
            );
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
        debug!("Transitioned from {prev_step:?} to {new_step:?}");
        if let Some(valid) = &state.driver.round_state.valid {
            if state.driver.step_is_propose() {
                info!(
                    "We enter Propose with a valid value from round {}",
                    valid.round
                );
            }
        }
        metrics.step_end(prev_step);
        metrics.step_start(new_step);
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
            let proposer = state.get_proposer(height, round)?;

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
                "Proposing value with id: {}, at round {}",
                proposal.value().id(),
                proposal.round()
            );

            let signed_proposal = state.ctx.sign_proposal(proposal);

            perform!(
                co,
                Effect::Broadcast(GossipMsg::Proposal(signed_proposal.clone()))
            );

            apply_driver_input(
                co,
                state,
                metrics,
                DriverInput::Proposal(signed_proposal.message, Validity::Valid),
            )
            .await
        }

        DriverOutput::Vote(vote) => {
            info!(
                "Voting {:?} for value {} at round {}",
                vote.vote_type(),
                PrettyVal(vote.value().as_ref()),
                vote.round()
            );

            let signed_vote = state.ctx.sign_vote(vote);

            perform!(co, Effect::Broadcast(GossipMsg::Vote(signed_vote.clone()),));

            apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote.message)).await
        }

        DriverOutput::Decide(round, value) => {
            // TODO: Remove proposal, votes, block for the round
            info!("Decided on value {}", value.id());

            // Store value decided on for retrieval when timeout commit elapses
            state
                .decision
                .insert((state.driver.height(), round), value.clone());

            perform!(co, Effect::ScheduleTimeout(Timeout::commit(round)));

            Ok(())
        }

        DriverOutput::ScheduleTimeout(timeout) => {
            info!("Scheduling {timeout}");

            perform!(co, Effect::ScheduleTimeout(timeout));

            Ok(())
        }

        DriverOutput::GetValue(height, round, timeout) => {
            info!("Requesting value at height {height} and round {round}");

            perform!(co, Effect::GetValue(height, round, timeout));

            Ok(())
        }
    }
}

async fn propose_value<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    round: Round,
    value: Ctx::Value,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.driver.height() != height {
        warn!(
            "Ignoring proposal for height {height}, current height: {}",
            state.driver.height()
        );

        return Ok(());
    }

    if state.driver.round() != round {
        warn!(
            "Ignoring propose value for round {round}, current round: {}",
            state.driver.round()
        );

        return Ok(());
    }

    apply_driver_input(co, state, metrics, DriverInput::ProposeValue(round, value)).await
}

async fn decide<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    round: Round,
    value: Ctx::Value,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // Remove the block information as it is not needed anymore
    state.remove_received_block(height, round);

    // Restore the commits. Note that they will be removed from `state`
    let commits = state.restore_precommits(height, round, &value);

    perform!(
        co,
        Effect::Decide {
            height,
            round,
            value,
            commits
        }
    );

    // Reinitialize to remove any previous round or equivocating precommits.
    // TODO: Revise when evidence module is added.
    state.signed_precommits.clear();

    metrics.block_end();
    metrics.finalized_blocks.inc();
    metrics
        .rounds_per_block
        .observe((round.as_i64() + 1) as f64);

    Ok(())
}

async fn on_vote<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    signed_vote: SignedVote<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.driver.height();
    let consensus_round = state.driver.round();
    let vote_height = signed_vote.height();
    let validator_address = signed_vote.validator_address();

    // Queue messages if driver is not initialized, or if they are for higher height.
    // Process messages received for the current height.
    // Drop all others.
    if consensus_round == Round::Nil {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote at round -1, queuing for later"
        );

        state.msg_queue.push_back(Msg::Vote(signed_vote));
        return Ok(());
    }

    if consensus_height < vote_height {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote for higher height, queuing for later"
        );

        state.msg_queue.push_back(Msg::Vote(signed_vote));
        return Ok(());
    }

    if consensus_height > vote_height {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote for lower height, dropping"
        );

        return Ok(());
    }

    info!(
        consensus.height = %consensus_height,
        vote.height = %vote_height,
        validator = %validator_address,
        "Received vote: {}", PrettyVote::<Ctx>(&signed_vote.message)
    );

    let Some(validator) = state.driver.validator_set.get_by_address(validator_address) else {
        warn!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote from unknown validator"
        );

        return Ok(());
    };

    let signed_msg = signed_vote.clone().map(ConsensusMsg::Vote);
    let verify_sig = Effect::VerifySignature(signed_msg, validator.public_key().clone());
    if !perform!(co, verify_sig, Resume::SignatureValidity(valid) => valid) {
        warn!(
            validator = %validator_address,
            "Received invalid vote: {}", PrettyVote::<Ctx>(&signed_vote.message)
        );

        return Ok(());
    }

    // Store the non-nil Precommits.
    if signed_vote.vote_type() == VoteType::Precommit && signed_vote.value().is_val() {
        state.store_signed_precommit(signed_vote.clone());
    }

    apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote.message)).await?;

    Ok(())
}

async fn on_proposal<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    signed_proposal: SignedProposal<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let proposal_height = signed_proposal.height();
    let proposal_round = signed_proposal.round();

    // Queue messages if driver is not initialized, or if they are for higher height.
    // Process messages received for the current height.
    // Drop all others.
    if state.driver.round() == Round::Nil {
        debug!("Received proposal at round -1, queuing for later");
        state.msg_queue.push_back(Msg::Proposal(signed_proposal));
        return Ok(());
    }

    if state.driver.height() < proposal_height {
        debug!("Received proposal for higher height, queuing for later");
        state.msg_queue.push_back(Msg::Proposal(signed_proposal));
        return Ok(());
    }

    if state.driver.height() > proposal_height {
        debug!("Received proposal for lower height, dropping");
        return Ok(());
    }

    let proposer_address = signed_proposal.validator_address();

    info!(%proposer_address, "Received proposal: {}", PrettyProposal::<Ctx>(&signed_proposal.message));

    let Some(proposer) = state.driver.validator_set.get_by_address(proposer_address) else {
        warn!(%proposer_address, "Received proposal from unknown validator");
        return Ok(());
    };

    let expected_proposer = state.get_proposer(proposal_height, proposal_round).unwrap();

    if expected_proposer != proposer_address {
        warn!(%proposer_address, % proposer_address, "Received proposal from a non-proposer");
        return Ok(());
    };

    if proposal_height != state.driver.height() {
        warn!(
            "Ignoring proposal for height {proposal_height}, current height: {}",
            state.driver.height()
        );

        return Ok(());
    }

    let signed_msg = signed_proposal.clone().map(ConsensusMsg::Proposal);
    let verify_sig = Effect::VerifySignature(signed_msg, proposer.public_key().clone());
    if !perform!(co, verify_sig, Resume::SignatureValidity(valid) => valid) {
        error!(
            "Received invalid signature for proposal: {}",
            PrettyProposal::<Ctx>(&signed_proposal.message)
        );

        return Ok(());
    }

    // Check if a complete block was received for the proposal POL round if defined or proposal round otherwise.
    let proposal_pol_round = signed_proposal.pol_round();
    let block_round = if proposal_pol_round.is_nil() {
        proposal_round
    } else {
        proposal_pol_round
    };

    let received_block = state
        .received_blocks
        .iter()
        .find(|(height, round, ..)| height == &proposal_height && round == &block_round);

    match received_block {
        Some((_height, _round, _value, valid)) => {
            apply_driver_input(
                co,
                state,
                metrics,
                DriverInput::Proposal(signed_proposal.message.clone(), *valid),
            )
            .await?;
        }
        None => {
            // Store the proposal and wait for all proposal parts
            info!(
                height = %signed_proposal.height(),
                round = %signed_proposal.round(),
                "Received proposal before all proposal parts, storing it"
            );

            // TODO - we should store the validity but we don't know it yet
            state
                .driver
                .proposal_keeper
                .apply_proposal(signed_proposal.message.clone());
        }
    }

    Ok(())
}

async fn on_timeout_elapsed<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    timeout: Timeout,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = state.driver.height();
    let round = state.driver.round();

    if timeout.round != round && timeout.step != TimeoutStep::Commit {
        debug!(
            "Ignoring timeout for round {} at height {}, current round: {round}",
            timeout.round, height
        );

        return Ok(());
    }

    info!("{timeout} elapsed at height {height} and round {round}");

    apply_driver_input(co, state, metrics, DriverInput::TimeoutElapsed(timeout)).await?;

    if timeout.step == TimeoutStep::Commit {
        let value = state
            .decision
            .remove(&(height, round))
            .ok_or_else(|| Error::DecidedValueNotFound(height, round))?;

        decide(co, state, metrics, height, round, value).await?;
    }

    Ok(())
}

#[tracing::instrument(
    skip_all,
    fields(
        height = %proposed_value.height,
        round = %proposed_value.round,
        validity = ?proposed_value.validity,
        id = %proposed_value.value.id()
    )
)]
async fn on_received_proposed_value<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposed_value: ProposedValue<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let ProposedValue {
        height,
        round,
        value,
        validity,
        ..
    } = proposed_value;

    // Store the block and validity information. It will be removed when a decision is reached for that height.
    state
        .received_blocks
        .push((height, round, value.clone(), validity));

    if let Some(proposal) = state.driver.proposal_keeper.get_proposal_for_round(round) {
        debug!(
            proposal.height = %proposal.height(),
            proposal.round = %proposal.round(),
            "We have a proposal for this round, checking..."
        );

        if height != proposal.height() {
            // The value we received is not for the current proposal, ignoring
            debug!("Proposed value is not for the current proposal, ignoring...");
            return Ok(());
        }

        let validity = Validity::from_bool(proposal.value() == &value && validity.is_valid());
        debug!("Applying proposal with validity {validity:?}");

        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::Proposal(proposal.clone(), validity),
        )
        .await?;
    } else {
        debug!("No proposal for this round yet, stored proposed value for later");
    }

    Ok(())
}

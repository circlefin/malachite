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
use crate::state::State;
use crate::types::GossipMsg;
use crate::util::pretty::{PrettyProposal, PrettyVal, PrettyVote};
use crate::ConsensusMsg;
use crate::{perform, ProposedValue};

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
        Msg::StartHeight(height, vs) => {
            reset_and_start_height(co, state, metrics, height, vs).await
        }
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
    validator_set: Ctx::ValidatorSet,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    perform!(co, Effect::CancelAllTimeouts);
    perform!(co, Effect::ResetTimeouts);

    metrics.step_end(state.driver.step());

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

        DriverInput::Proposal(proposal, validity) => {
            if proposal.height() != state.driver.height() {
                warn!(
                    "Ignoring proposal for height {}, current height: {}",
                    proposal.height(),
                    state.driver.height()
                );

                return Ok(());
            }

            // Store the proposal
            state
                .driver
                .proposal_keeper
                .apply_proposal(proposal.clone(), *validity);

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

            on_proposal(co, state, metrics, signed_proposal).await
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

            apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote)).await
        }

        DriverOutput::Decide(consensus_round, proposal) => {
            // TODO: Remove proposal, votes, block for the round
            info!(
                "Decided in round {} on proposal {:?}",
                consensus_round, proposal
            );

            // Store value decided on for retrieval when timeout commit elapses
            state
                .decision
                .insert((state.driver.height(), consensus_round), proposal.clone());

            perform!(
                co,
                Effect::ScheduleTimeout(Timeout::commit(consensus_round))
            );

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

    state.store_value(&ProposedValue {
        height,
        round,
        validator_address: state.driver.address.clone(),
        value: value.clone(),
        validity: Validity::Valid,
    });

    apply_driver_input(co, state, metrics, DriverInput::ProposeValue(round, value)).await
}

async fn decide<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    consensus_round: Round,
    proposal: Ctx::Proposal,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = proposal.height();
    let proposal_round = proposal.round();
    let value = proposal.value();

    // Restore the commits. Note that they will be removed from `state`
    let commits = state.restore_precommits(height, proposal_round, value);

    // Clean proposals and values
    state.remove_full_proposals(height);

    perform!(
        co,
        Effect::Decide {
            height,
            round: proposal_round,
            value: value.clone(),
            commits
        }
    );

    // Reinitialize to remove any previous round or equivocating precommits.
    // TODO: Revise when evidence module is added.
    state.signed_precommits.clear();

    metrics.block_end();
    metrics.finalized_blocks.inc();

    metrics
        .consensus_round
        .observe(consensus_round.as_i64() as f64);

    metrics
        .proposal_round
        .observe(proposal_round.as_i64() as f64);

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

    // Store the non-nil Precommits.
    if signed_vote.vote_type() == VoteType::Precommit && signed_vote.value().is_val() {
        state.store_signed_precommit(signed_vote.clone());
    }

    apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote)).await?;

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

    let signed_msg = signed_proposal.clone().map(ConsensusMsg::Proposal);
    let verify_sig = Effect::VerifySignature(signed_msg, proposer.public_key().clone());
    if !perform!(co, verify_sig, Resume::SignatureValidity(valid) => valid) {
        error!(
            "Received invalid signature for proposal: {}",
            PrettyProposal::<Ctx>(&signed_proposal.message)
        );

        return Ok(());
    }

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

    if proposal_height != state.driver.height() {
        warn!(
            "Ignoring proposal for height {proposal_height}, current height: {}",
            state.driver.height()
        );

        return Ok(());
    }

    state.store_proposal(signed_proposal.clone());

    if let Some(full_proposal) = state.full_proposal_at_round_and_value(
        &proposal_height,
        proposal_round,
        signed_proposal.value(),
    ) {
        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::Proposal(full_proposal.proposal.clone(), full_proposal.validity),
        )
        .await?;
    } else {
        debug!(
            proposal.height = %proposal_height,
            proposal.round = %proposal_round,
            "No full proposal for this round yet, stored proposal for later"
        );
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
        let proposal = state
            .decision
            .remove(&(height, round))
            .ok_or_else(|| Error::DecidedValueNotFound(height, round))?;

        decide(co, state, metrics, round, proposal).await?;
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
    if state.driver.height() > proposed_value.height {
        debug!("Received value for lower height, dropping");
        return Ok(());
    }

    if state.driver.height() < proposed_value.height {
        debug!("Received value for higher height, queuing for later");
        state
            .msg_queue
            .push_back(Msg::ReceivedProposedValue(proposed_value));
        return Ok(());
    }

    state.store_value(&proposed_value);

    let proposals = state.full_proposals_for_value(&proposed_value);
    for signed_proposal in proposals {
        debug!(
            proposal.height = %signed_proposal.height(),
            proposal.round = %signed_proposal.round(),
            "We have a full proposal for this round, checking..."
        );

        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::Proposal(signed_proposal, proposed_value.validity),
        )
        .await?;
    }

    Ok(())
}

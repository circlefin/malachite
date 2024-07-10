use tracing::{debug, error, info, trace, warn};

use malachite_common::*;
use malachite_driver::Input as DriverInput;
use malachite_driver::Output as DriverOutput;
use malachite_metrics::Metrics;

use crate::mock::{Block, GossipEvent, NetworkMsg, PeerId};
use crate::util::pretty::{PrettyProposal, PrettyVal, PrettyVote};
use crate::{emit, emit_then, Effect, Error, Msg, Resume, State, Yielder};

pub fn handle_msg<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    msg: Msg<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match msg {
        Msg::StartHeight(height) => start_height(state, metrics, yielder, height),
        Msg::MoveToHeight(height) => move_to_height(state, metrics, yielder, height),
        Msg::GossipEvent(event) => on_gossip_event(state, metrics, yielder, event),
        Msg::TimeoutElapsed(timeout) => on_timeout_elapsed(state, metrics, yielder, timeout),
        Msg::ReceivedBlock(block) => on_received_block(state, metrics, yielder, block),
        Msg::GossipBlockPart(_) => todo!(),
    }
}

fn start_height<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let round = Round::new(0);
    info!("Starting new height {height} at round {round}");

    let proposer = state.get_proposer(height, round).cloned()?;
    info!("Proposer for height {height} and round {round}: {proposer}");

    apply_driver_input(
        state,
        metrics,
        yielder,
        DriverInput::NewRound(height, round, proposer),
    )?;

    metrics.block_start();
    metrics.height.set(height.as_u64() as i64);
    metrics.round.set(round.as_i64());

    replay_pending_msgs(state, metrics, yielder)?;

    Ok(())
}

fn move_to_height<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    emit!(yielder, Effect::CancelAllTimeouts);
    emit!(yielder, Effect::ResetTimeouts);

    // End the current step (most likely Commit)
    metrics.step_end(state.driver.step());

    let validator_set = emit_then!(yielder, Effect::GetValidatorSet(height),
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

    handle_msg(state, metrics, yielder, Msg::StartHeight(height))
}

fn replay_pending_msgs<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let pending_msgs = std::mem::take(&mut state.msg_queue);
    debug!("Replaying {} messages", pending_msgs.len());

    for pending_msg in pending_msgs {
        handle_msg(state, metrics, yielder, pending_msg)?;
    }

    Ok(())
}

fn apply_driver_input<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    input: DriverInput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match &input {
        DriverInput::NewRound(_, _, _) => {
            emit!(yielder, Effect::CancelAllTimeouts);
        }

        DriverInput::ProposeValue(round, _) => {
            emit!(yielder, Effect::CancelTimeout(Timeout::propose(*round)));
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

            if proposal.round() != state.driver.round() {
                warn!(
                    "Ignoring proposal for round {}, current round: {}",
                    proposal.round(),
                    state.driver.round()
                );

                return Ok(());
            }

            emit!(
                yielder,
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

            if vote.round() != state.driver.round() {
                warn!(
                    "Ignoring vote for round {}, current round: {}",
                    vote.round(),
                    state.driver.round()
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

        metrics.step_end(prev_step);
        metrics.step_start(new_step);
    }

    process_driver_outputs(state, metrics, yielder, outputs)?;

    Ok(())
}

fn process_driver_outputs<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    outputs: Vec<DriverOutput<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    for output in outputs {
        process_driver_output(state, metrics, yielder, output)?;
    }

    Ok(())
}

fn process_driver_output<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    output: DriverOutput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match output {
        DriverOutput::NewRound(height, round) => {
            info!("Starting round {round} at height {height}");
            metrics.round.set(round.as_i64());

            let proposer = state.get_proposer(height, round)?;
            info!("Proposer for height {height} and round {round}: {proposer}");

            apply_driver_input(
                state,
                metrics,
                yielder,
                DriverInput::NewRound(height, round, proposer.clone()),
            )
        }

        DriverOutput::Propose(proposal) => {
            info!(
                "Proposing value with id: {}, at round {}",
                proposal.value().id(),
                proposal.round()
            );

            let signed_proposal = state.ctx.sign_proposal(proposal);

            emit!(
                yielder,
                Effect::Broadcast(
                // TODO: Define full Broadcast variant
                // Channel::Consensus,
                // NetworkMsg::Proposal(signed_proposal.clone()),
            )
            );

            apply_driver_input(
                state,
                metrics,
                yielder,
                DriverInput::Proposal(signed_proposal.proposal, Validity::Valid),
            )
        }

        DriverOutput::Vote(vote) => {
            info!(
                "Voting {:?} for value {} at round {}",
                vote.vote_type(),
                PrettyVal(vote.value().as_ref()),
                vote.round()
            );

            let signed_vote = state.ctx.sign_vote(vote);

            emit!(
                yielder,
                Effect::Broadcast(
                // TODO: Implement Broadcast variant
                // Channel::Consensus,
                // NetworkMsg::Vote(signed_vote.clone()),
            )
            );

            apply_driver_input(state, metrics, yielder, DriverInput::Vote(signed_vote.vote))
        }

        DriverOutput::Decide(round, value) => {
            // TODO: Remove proposal, votes, block for the round
            info!("Decided on value {}", value.id());

            emit!(yielder, Effect::ScheduleTimeout(Timeout::commit(round)));

            decided(state, metrics, yielder, state.driver.height(), round, value)
        }

        DriverOutput::ScheduleTimeout(timeout) => {
            info!("Scheduling {timeout}");

            emit!(yielder, Effect::ScheduleTimeout(timeout));

            Ok(())
        }

        DriverOutput::GetValue(height, round, timeout) => {
            info!("Requesting value at height {height} and round {round}");

            let (height, round, value) = emit_then!(yielder, Effect::GetValue(height, round, timeout),
                Resume::ProposeValue(height, round, value) => (height, round, value)
            );

            propose_value(state, metrics, yielder, height, round, value)
        }
    }
}

fn propose_value<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
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
            "Ignoring proposal for round {round}, current round: {}",
            state.driver.round()
        );

        return Ok(());
    }

    apply_driver_input(
        state,
        metrics,
        yielder,
        DriverInput::ProposeValue(round, value),
    )
}

fn decided<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
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

    emit!(
        yielder,
        Effect::DecidedOnValue {
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

fn on_gossip_event<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    event: GossipEvent<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match event {
        GossipEvent::Listening(addr) => {
            info!("Listening on {addr}");
            Ok(())
        }

        GossipEvent::PeerConnected(peer_id) => {
            if !state.connected_peers.insert(peer_id) {
                // We already saw that peer, ignoring...
                return Ok(());
            }

            info!("Connected to peer {peer_id}");

            metrics.connected_peers.inc();

            if state.connected_peers.len() == state.driver.validator_set.count() - 1 {
                info!(
                    "Enough peers ({}) connected to start consensus",
                    state.connected_peers.len()
                );

                handle_msg(
                    state,
                    metrics,
                    yielder,
                    Msg::StartHeight(state.driver.height()),
                )?;
            }

            Ok(())
        }

        GossipEvent::PeerDisconnected(peer_id) => {
            info!("Disconnected from peer {peer_id}");

            if state.connected_peers.remove(&peer_id) {
                metrics.connected_peers.dec();

                // TODO: pause/stop consensus, if necessary
            }

            Ok(())
        }

        GossipEvent::Message(from, msg) => {
            let Some(msg_height) = msg.msg_height() else {
                trace!("Received message without height, dropping");
                return Ok(());
            };

            // Queue messages if driver is not initialized, or if they are for higher height.
            // Process messages received for the current height.
            // Drop all others.
            if state.driver.round() == Round::Nil {
                debug!("Received gossip event at round -1, queuing for later");

                state
                    .msg_queue
                    .push_back(Msg::GossipEvent(GossipEvent::Message(from, msg)));
            } else if state.driver.height() < msg_height {
                debug!("Received gossip event for higher height, queuing for later");

                state
                    .msg_queue
                    .push_back(Msg::GossipEvent(GossipEvent::Message(from, msg)));
            } else if state.driver.height() == msg_height {
                on_network_msg(state, metrics, yielder, from, msg)?;
            }

            Ok(())
        }
    }
}

fn on_network_msg<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    from: PeerId,
    msg: NetworkMsg<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match msg {
        NetworkMsg::Vote(signed_vote) => {
            let validator_address = signed_vote.validator_address();

            info!(%from, validator = %validator_address, "Received vote: {}", PrettyVote::<Ctx>(&signed_vote.vote));

            if signed_vote.vote.height() != state.driver.height() {
                warn!(
                    %from, validator = %validator_address,
                    "Ignoring vote for height {}, current height: {}",
                    signed_vote.vote.height(),
                    state.driver.height()
                );

                return Ok(());
            }

            let Some(validator) = state.driver.validator_set.get_by_address(validator_address)
            else {
                warn!(
                    %from, validator = %validator_address,
                    "Received vote from unknown validator"
                );

                return Ok(());
            };

            if !state
                .ctx
                .verify_signed_vote(&signed_vote, validator.public_key())
            {
                warn!(
                    %from, validator = %validator_address,
                    "Received invalid vote: {}", PrettyVote::<Ctx>(&signed_vote.vote)
                );

                return Ok(());
            }

            // Store the non-nil Precommits.
            if signed_vote.vote.vote_type() == VoteType::Precommit
                && signed_vote.vote.value().is_val()
            {
                state.store_signed_precommit(signed_vote.clone());
            }

            apply_driver_input(state, metrics, yielder, DriverInput::Vote(signed_vote.vote))?;
        }

        NetworkMsg::Proposal(signed_proposal) => {
            let validator = signed_proposal.proposal.validator_address();

            info!(%from, %validator, "Received proposal: {}", PrettyProposal::<Ctx>(&signed_proposal.proposal));

            let Some(validator) = state.driver.validator_set.get_by_address(validator) else {
                warn!(%from, %validator, "Received proposal from unknown validator");
                return Ok(());
            };

            let proposal = &signed_proposal.proposal;
            let proposal_height = proposal.height();
            let proposal_round = proposal.round();

            if proposal_height != state.driver.height() {
                warn!(
                    "Ignoring proposal for height {proposal_height}, current height: {}",
                    state.driver.height()
                );

                return Ok(());
            }

            if proposal_round != state.driver.round() {
                warn!(
                    "Ignoring proposal for round {proposal_round}, current round: {}",
                    state.driver.round()
                );

                return Ok(());
            }

            if !state
                .ctx
                .verify_signed_proposal(&signed_proposal, validator.public_key())
            {
                error!(
                    "Received invalid signature for proposal: {}",
                    PrettyProposal::<Ctx>(&signed_proposal.proposal)
                );

                return Ok(());
            }

            let received_block = state
                .received_blocks
                .iter()
                .find(|(height, round, ..)| height == &proposal_height && round == &proposal_round);

            match received_block {
                Some((_height, _round, _value, valid)) => {
                    apply_driver_input(
                        state,
                        metrics,
                        yielder,
                        DriverInput::Proposal(proposal.clone(), *valid),
                    )?;
                }
                None => {
                    // Store the proposal and wait for all block parts
                    // TODO: or maybe integrate with receive-proposal() here? will this block until all parts are received?

                    info!(
                        height = %proposal.height(),
                        round = %proposal.round(),
                        "Received proposal before all block parts, storing it"
                    );

                    // FIXME: Avoid mutating the driver state directly
                    state.driver.proposal = Some(proposal.clone());
                }
            }
        }

        NetworkMsg::BlockPart(signed_block_part) => {
            let validator_address = signed_block_part.validator_address();

            let Some(validator) = state.driver.validator_set.get_by_address(validator_address)
            else {
                warn!(%from, %validator_address, "Received block part from unknown validator");
                return Ok(());
            };

            if !state
                .ctx
                .verify_signed_block_part(&signed_block_part, validator.public_key())
            {
                warn!(%from, validator = %validator_address, "Received invalid block part: {signed_block_part:?}");
                return Ok(());
            }

            // TODO: Verify that the proposal was signed by the proposer for the height and round, drop otherwise.
            emit!(
                yielder,
                Effect::ReceivedBlockPart(signed_block_part.block_part)
            );
        }
    }

    Ok(())
}

fn on_timeout_elapsed<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    timeout: Timeout,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = state.driver.height();
    let round = state.driver.round();

    if timeout.round != round {
        debug!(
            "Ignoring timeout for round {} at height {}, current round: {round}",
            timeout.round, height
        );

        return Ok(());
    }

    info!("{timeout} elapsed at height {height} and round {round}");

    apply_driver_input(
        state,
        metrics,
        yielder,
        DriverInput::TimeoutElapsed(timeout),
    )?;

    if timeout.step == TimeoutStep::Commit {
        handle_msg(
            state,
            metrics,
            yielder,
            Msg::MoveToHeight(height.increment()),
        )?;
    }

    Ok(())
}

fn on_received_block<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder<Ctx>,
    block: Block<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let Block {
        height,
        round,
        value,
        validity,
        ..
    } = block;

    info!(%height, %round, "Received block: {}", value.id());

    // Store the block and validity information. It will be removed when a decision is reached for that height.
    state
        .received_blocks
        .push((height, round, value.clone(), validity));

    if let Some(proposal) = state.driver.proposal.as_ref() {
        if height != proposal.height() || round != proposal.round() {
            // The block we received is not for the current proposal, ignoring
            return Ok(());
        }

        let validity = Validity::from_bool(proposal.value() == &value && validity.is_valid());

        apply_driver_input(
            state,
            metrics,
            yielder,
            DriverInput::Proposal(proposal.clone(), validity),
        )?;
    }

    Ok(())
}

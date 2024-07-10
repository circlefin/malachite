use corosensei::stack::DefaultStack;
use corosensei::ScopedCoroutine;
use tracing::{debug, error, info, warn};

use malachite_common::*;
use malachite_driver::Input as DriverInput;
use malachite_driver::Output as DriverOutput;
use malachite_metrics::Metrics;

use crate::util::pretty::PrettyVal;
use crate::{Error, Msg, State};

pub type Gen<'a, Ctx> = ScopedCoroutine<'a, (), Yield, Result<(), Error<Ctx>>, DefaultStack>;
pub type Yielder = corosensei::Yielder<(), Yield>;

pub enum Yield {
    CancelAllTimeouts,
    CancelTimeout(Timeout),
    ScheduleTimeout(Timeout),
    Broadcast(),
    GetValue(<Ctx as Context>::Height, Round, Timeout),
}

pub fn handle<'a, Ctx>(
    state: &'a mut State<Ctx>,
    metrics: &'a Metrics,
    msg: Msg<Ctx>,
) -> Gen<'a, Ctx>
where
    Ctx: Context,
{
    ScopedCoroutine::new(|yielder, ()| handle_inner(state, metrics, yielder, msg))
}

fn handle_inner<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder,
    msg: Msg<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match msg {
        Msg::StartHeight(height) => {
            metrics.block_start();

            let round = Round::new(0);
            info!("Starting new height {height} at round {round}");

            let proposer = state.get_proposer(height, round).cloned()?;

            info!("Proposer for height {height} and round {round}: {proposer}");

            metrics.height.set(height.as_u64() as i64);
            metrics.round.set(round.as_i64());

            apply_driver_input(
                state,
                metrics,
                yielder,
                DriverInput::NewRound(height, round, proposer),
            )?;

            replay_pending_msgs(state, metrics, yielder)?;
        }

        Msg::MoveToHeight(height) => todo!(),
        Msg::GossipEvent(_) => todo!(),
        Msg::TimeoutElapsed(_) => todo!(),
        Msg::ApplyDriverInput(_) => todo!(),
        Msg::Decided(_, _, _) => todo!(),
        Msg::ProcessDriverOutputs(_) => todo!(),
        Msg::ProposeValue(_, _, _) => todo!(),
        Msg::GossipBlockPart(_) => todo!(),
        Msg::ProposalReceived(_) => todo!(),
    }

    Ok(())
}

fn replay_pending_msgs<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let pending_msgs = std::mem::take(&mut state.msg_queue);
    debug!("Replaying {} messages", pending_msgs.len());

    for pending_msg in pending_msgs {
        handle_inner(state, metrics, yielder, pending_msg)?;
    }

    Ok(())
}

fn apply_driver_input<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder,
    input: DriverInput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match &input {
        DriverInput::NewRound(_, _, _) => {
            yielder.suspend(Yield::CancelAllTimeouts);
        }

        DriverInput::ProposeValue(round, _) => {
            yielder.suspend(Yield::CancelTimeout(Timeout::propose(*round)));
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

            yielder.suspend(Yield::CancelTimeout(Timeout::propose(proposal.round())));
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
        .map_err(|e| format!("Driver failed to process input: {e}"));

    // Record the step we are now at
    let new_step = state.driver.step();

    // If the step has changed, update the metrics
    if prev_step != new_step {
        debug!("Transitioned from {prev_step:?} to {new_step:?}");

        metrics.step_end(prev_step);
        metrics.step_start(new_step);
    }

    match outputs {
        Ok(outputs) => handle_inner(state, metrics, yielder, Msg::ProcessDriverOutputs(outputs))?,
        Err(error) => error!("{error}"),
    }

    Ok(())
}

fn process_driver_outputs<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    yielder: &Yielder,
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
    yielder: &Yielder,
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

            yielder.suspend(Yield::Broadcast(
                // TODO: Define full Broadcast variant
                // Channel::Consensus,
                // NetworkMsg::Proposal(signed_proposal.clone()),
            ));

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

            yielder.suspend(Yield::Broadcast(
                // TODO: Implement Broadcast variant
                // Channel::Consensus,
                // NetworkMsg::Vote(signed_vote.clone()),
            ));

            apply_driver_input(state, metrics, yielder, DriverInput::Vote(signed_vote.vote))
        }

        DriverOutput::Decide(round, value) => {
            // TODO: Remove proposal, votes, block for the round
            info!("Decided on value {}", value.id());

            yielder.suspend(Yield::ScheduleTimeout(Timeout::commit(round)));

            handle_inner(
                state,
                metrics,
                yielder,
                Msg::Decided(state.driver.height(), round, value),
            )
        }

        DriverOutput::ScheduleTimeout(timeout) => {
            info!("Scheduling {timeout}");

            yielder.suspend(Yield::ScheduleTimeout(timeout));

            Ok(())
        }

        DriverOutput::GetValue(height, round, timeout) => {
            info!("Requesting value at height {height} and round {round}");

            let value = yielder.suspend(Yield::GetValue(height, round, timeout));

            // TODO: Handle value

            Ok(())
        }
    }
}

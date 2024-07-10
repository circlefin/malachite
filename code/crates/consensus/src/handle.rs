use tracing::{debug, info};

use malachite_common::*;
use malachite_driver::Input as DriverInput;
use malachite_metrics::Metrics;

use crate::{Error, Msg, State};

pub fn handle<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
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

            let proposer = state
                .get_proposer(height, round, &state.validator_set)
                .cloned()?;

            info!("Proposer for height {height} and round {round}: {proposer}");

            metrics.height.set(height.as_u64() as i64);
            metrics.round.set(round.as_i64());

            apply_driver_input(
                state,
                metrics,
                DriverInput::NewRound(height, round, proposer),
            )?;

            replay_pending_msgs(state, metrics)?;
        }

        Msg::MoveToHeight(_) => todo!(),
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

fn replay_pending_msgs<Ctx>(state: &mut State<Ctx>, metrics: &Metrics) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let pending_msgs = std::mem::take(&mut state.msg_queue);
    debug!("Replaying {} messages", pending_msgs.len());

    for pending_msg in pending_msgs {
        handle(state, metrics, pending_msg)?;
    }

    Ok(())
}

fn apply_driver_input<Ctx>(
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposer: DriverInput<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    todo!()
}

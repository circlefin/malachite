use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::handle::handle_input;

pub async fn reset_and_start_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    validator_set: Ctx::ValidatorSet,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    perform!(co, Effect::CancelAllTimeouts(Default::default()));
    perform!(co, Effect::ResetTimeouts(Default::default()));

    #[cfg(feature = "metrics")]
    metrics.step_end(state.driver.step());

    state.driver.move_to_height(height, validator_set);

    debug_assert_eq!(state.driver.height(), height);
    debug_assert_eq!(state.driver.round(), Round::Nil);

    start_height(co, state, metrics, height).await
}

pub async fn start_height<Ctx>(
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

    #[cfg(feature = "metrics")]
    {
        metrics.block_start();
        metrics.height.set(height.as_u64() as i64);
        metrics.round.set(round.as_i64());
    }

    let proposer = state.get_proposer(height, round);

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::NewRound(height, round, proposer.clone()),
    )
    .await?;

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
    let pending_inputs = std::mem::take(&mut state.input_queue);
    debug!(count = pending_inputs.len(), "Replaying inputs");

    for pending_input in pending_inputs {
        handle_input(co, state, metrics, pending_input).await?;
    }

    Ok(())
}

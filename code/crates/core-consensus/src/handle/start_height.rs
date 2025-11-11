use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::handle::handle_input;

pub async fn reset_and_start_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    validator_set: Option<Ctx::ValidatorSet>,
    is_restart: bool,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    perform!(co, Effect::CancelAllTimeouts(Default::default()));

    #[cfg(feature = "metrics")]
    metrics.step_end(state.driver.step());

    state.reset_and_start_height(height, validator_set);

    debug_assert_eq!(state.height(), height);
    debug_assert_eq!(state.round(), Round::Nil);

    on_start_height(co, state, metrics, height, is_restart).await
}

async fn on_start_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    is_restart: bool,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug_assert_eq!(state.height(), height);
    debug_assert_eq!(state.round(), Round::Nil);

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

    replay_pending_msgs(co, state, metrics, is_restart).await?;

    Ok(())
}

async fn replay_pending_msgs<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    is_restart: bool,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // Take all inputs that are pending for the current height.
    let pending_inputs = state.take_pending_inputs(metrics);

    if is_restart {
        return Ok(());
    }

    debug!(count = pending_inputs.len(), "Replaying inputs");

    for pending_input in pending_inputs {
        handle_input(co, state, metrics, pending_input).await?;
    }

    Ok(())
}

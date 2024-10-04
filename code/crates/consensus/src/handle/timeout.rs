use crate::handle::decide::decide;
use crate::handle::driver::apply_driver_input;
use crate::prelude::*;

pub async fn on_timeout_elapsed<Ctx>(
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

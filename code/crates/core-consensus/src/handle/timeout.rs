use crate::handle::decide::decide;
use crate::handle::driver::apply_driver_input;
use crate::handle::rebroadcast_timeout::on_rebroadcast_timeout;
use crate::handle::step_timeout::on_step_limit_timeout;
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

    if timeout.round != round && timeout.kind != TimeoutKind::Commit {
        debug!(
            %height,
            %round,
            timeout.round = %timeout.round,
            "Ignoring timeout for different round",
        );

        return Ok(());
    }

    info!(
        step = ?timeout.kind,
        %timeout.round,
        %height,
        %round,
        "Timeout elapsed"
    );

    if matches!(
        timeout.kind,
        TimeoutKind::Propose | TimeoutKind::Prevote | TimeoutKind::Precommit | TimeoutKind::Commit
    ) {
        // Persist the timeout in the Write-ahead Log.
        // Time-limit and rebroadcast timeouts are not persisted because they only occur when consensus is stuck.
        perform!(co, Effect::WalAppendTimeout(timeout, Default::default()));
    }

    apply_driver_input(co, state, metrics, DriverInput::TimeoutElapsed(timeout)).await?;

    match timeout.kind {
        TimeoutKind::PrevoteRebroadcast | TimeoutKind::PrecommitRebroadcast => {
            on_rebroadcast_timeout(co, state, metrics, timeout).await
        }
        TimeoutKind::PrevoteTimeLimit | TimeoutKind::PrecommitTimeLimit => {
            on_step_limit_timeout(co, state, metrics, timeout.round).await
        }
        TimeoutKind::Commit => {
            let proposal = state
                .decision
                .remove(&(height, round))
                .ok_or_else(|| Error::DecidedValueNotFound(height, round))?;

            decide(co, state, metrics, round, proposal).await
        }
        _ => Ok(()),
    }
}

use crate::prelude::*;

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn on_step_limit_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    warn!(
        height = %state.driver.height(), round = %state.driver.round(),
        "Consensus is halted in {:?} step, start vote synchronization", state.driver.step());

    perform!(
        co,
        Effect::GetVoteSet(state.driver.height(), round, Default::default())
    );

    #[cfg(feature = "metrics")]
    metrics.step_timeouts.inc();

    if state.driver.step_is_prevote() {
        perform!(
            co,
            Effect::ScheduleTimeout(
                Timeout::prevote_time_limit(state.driver.round()),
                Default::default()
            )
        );
    }

    if state.driver.step_is_precommit() {
        perform!(
            co,
            Effect::ScheduleTimeout(
                Timeout::precommit_time_limit(state.driver.round()),
                Default::default()
            )
        );
    }

    Ok(())
}

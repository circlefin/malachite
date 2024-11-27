use crate::prelude::*;

pub async fn on_step_limit_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // TODO - Update metrics

    debug!(
        "on_step_limit_timeout {:?} {} {}",
        state.driver.step(),
        state.driver.height(),
        round
    );

    perform!(co, Effect::GetVoteSet(state.driver.height(), round));

    if state.driver.step_is_prevote() {
        debug!("VS1 - node has stayed too long in the prevote step");

        perform!(
            co,
            Effect::ScheduleTimeout(Timeout::prevote_time_limit(state.driver.round()))
        );
    }

    if state.driver.step_is_precommit() {
        debug!("VS1 - node has stayed too long in precommit step");

        perform!(
            co,
            Effect::ScheduleTimeout(Timeout::precommit_time_limit(state.driver.round()))
        );
    }

    Ok(())
}

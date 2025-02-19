use crate::prelude::*;

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn on_rebroadcast_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    timeout: Timeout,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let (height, round) = (state.driver.height(), state.driver.round());

    let (maybe_vote, timeout) = match timeout.kind {
        TimeoutKind::PrevoteRebroadcast => (
            state.last_prevote.as_ref(),
            Timeout::prevote_rebroadcast(round),
        ),
        TimeoutKind::PrecommitRebroadcast => (
            state.last_precommit.as_ref(),
            Timeout::precommit_rebroadcast(round),
        ),
        _ => return Ok(()),
    };

    if let Some(vote) = maybe_vote.cloned() {
        warn!(
            %height, %round,
            "Rebroadcasting vote at {:?} step after {:?} timeout",
            state.driver.step(), timeout.kind,
        );

        perform!(co, Effect::Rebroadcast(vote, Default::default()));
        perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));
    }

    #[cfg(feature = "metrics")]
    metrics.rebroadcast_timeouts.inc();

    Ok(())
}

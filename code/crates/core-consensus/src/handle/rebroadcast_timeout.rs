use crate::{prelude::*, SignedConsensusMsg};

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
    let (maybe_vote, timeout) = match timeout.kind {
        TimeoutKind::PrevoteRebroadcast => (
            &state.last_prevote,
            Timeout::prevote_rebroadcast(state.driver.round()),
        ),
        TimeoutKind::PrecommitRebroadcast => (
            &state.last_precommit,
            Timeout::precommit_rebroadcast(state.driver.round()),
        ),
        _ => return Ok(()),
    };

    if let Some(vote) = maybe_vote {
        warn!(
            height = %state.driver.height(), round = %state.driver.round(),
            "{:?} {:?} step, vote rebroadcast", timeout.kind, state.driver.step());

        perform!(
            co,
            Effect::Rebroadcast(SignedConsensusMsg::Vote(vote.clone()), Default::default())
        );
        perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));
    }

    #[cfg(feature = "metrics")]
    metrics.rebroadcast_timeouts.inc();

    Ok(())
}

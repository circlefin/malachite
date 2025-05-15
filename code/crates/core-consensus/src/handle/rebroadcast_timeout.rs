use crate::{prelude::*, VoteSyncMode};

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn on_rebroadcast_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.params.vote_sync_mode != VoteSyncMode::Rebroadcast {
        return Ok(());
    }

    let (height, round) = (state.driver.height(), state.driver.round());

    if let Some(vote) = state.last_signed_prevote.as_ref() {
        warn!(
            %height, %round, vote_height = %vote.height(), vote_round = %vote.round(),
            "Rebroadcasting vote at {:?} step",
            state.driver.step()
        );

        perform!(
            co,
            Effect::RebroadcastVote(vote.clone(), Default::default())
        );
    };

    if let Some(vote) = state.last_signed_precommit.as_ref() {
        warn!(
            %height, %round, vote_height = %vote.height(), vote_round = %vote.round(),
            "Rebroadcasting vote at {:?} step",
            state.driver.step()
        );
        perform!(
            co,
            Effect::RebroadcastVote(vote.clone(), Default::default())
        );
    };

    if let Some(local) = state.round_certificate() {
        if local.target_round == round {
            warn!(
                %local.certificate.height,
                %round,
                %local.certificate.round,
                number_of_votes = local.certificate.round_signatures.len(),
                "Rebroadcasting round certificate"
            );
            perform!(
                co,
                Effect::RebroadcastRoundCertificate(local.certificate.clone(), Default::default())
            );
        }
    }

    #[cfg(feature = "metrics")]
    metrics.rebroadcast_timeouts.inc();

    let timeout = Timeout::rebroadcast(round);
    perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));

    Ok(())
}

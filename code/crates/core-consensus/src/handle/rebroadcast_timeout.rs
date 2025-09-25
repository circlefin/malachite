use crate::prelude::*;

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn on_rebroadcast_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let (height, round) = (state.height(), state.round());

    // Only rebroadcast if we're an active validator
    if state.is_active_validator() {
        if let Some(vote) = state.last_signed_prevote.as_ref() {
            warn!(
                %height, %round, vote_height = %vote.height(), vote_round = %vote.round(),
                "Rebroadcasting vote at {:?} step",
                state.driver.step()
            );

            perform!(co, Effect::RepublishVote(vote.clone(), Default::default()));
        };

        if let Some(vote) = state.last_signed_precommit.as_ref() {
            warn!(
                %height, %round, vote_height = %vote.height(), vote_round = %vote.round(),
                "Rebroadcasting vote at {:?} step",
                state.driver.step()
            );
            perform!(co, Effect::RepublishVote(vote.clone(), Default::default()));
        };

        if let Some(cert) = state.round_certificate() {
            if cert.enter_round == round {
                warn!(
                    %cert.certificate.height,
                    %round,
                    %cert.certificate.round,
                    number_of_votes = cert.certificate.round_signatures.len(),
                    "Rebroadcasting round certificate"
                );
                perform!(
                    co,
                    Effect::RepublishRoundCertificate(cert.certificate.clone(), Default::default())
                );
            }
        };
    }

    #[cfg(feature = "metrics")]
    metrics.rebroadcast_timeouts.inc();

    let timeout = Timeout::rebroadcast(round);
    perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));

    Ok(())
}

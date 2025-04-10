use tracing::error;

use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_polka_certificate;
use crate::handle::validator_set::get_validator_set;
use crate::handle::vote::on_vote;
use crate::input::RequestId;
use crate::prelude::*;

pub async fn on_vote_set_request<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    request_id: RequestId,
    height: Ctx::Height,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%height, %round, %request_id, "Received vote set request, retrieve the votes and send response if set is not empty");

    if let Some((votes, polka_certificate)) = state.restore_votes(height, round) {
        perform!(
            co,
            Effect::SendVoteSetResponse(
                request_id,
                height,
                round,
                VoteSet::new(votes),
                polka_certificate,
                Default::default()
            )
        );
    }

    Ok(())
}

pub async fn on_vote_set_response<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    vote_set: VoteSet<Ctx>,
    certificates: Vec<PolkaCertificate<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        height = %state.height(), round = %state.round(),
        votes.count = %vote_set.len(),
        polka_certificates.count = %certificates.len(),
        "Received vote set response"
    );

    apply_polka_certificates(co, state, metrics, certificates).await?;

    for vote in vote_set.votes {
        on_vote(co, state, metrics, vote).await?;
    }

    Ok(())
}

async fn apply_polka_certificates<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificates: Vec<PolkaCertificate<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    for certificate in certificates {
        if let Err(e) = apply_polka_certificate(co, state, metrics, certificate).await {
            error!("Failed to apply polka certificate: {e}");
        }
    }

    Ok(())
}

async fn apply_polka_certificate<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificate: PolkaCertificate<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if certificate.height != state.height() {
        warn!(
            %certificate.height,
            consensus.height = %state.height(),
            "Polka certificate height mismatch"
        );

        return Ok(());
    }

    let validator_set = get_validator_set(co, state, certificate.height)
        .await?
        .ok_or_else(|| Error::ValidatorSetNotFound(certificate.height))?;

    let validity = verify_polka_certificate(
        co,
        certificate.clone(),
        validator_set.into_owned(),
        state.params.threshold_params,
    )
    .await?;

    if let Err(e) = validity {
        warn!(?certificate, "Invalid polka certificate: {e}");
        return Ok(());
    }

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::PolkaCertificate(certificate),
    )
    .await
}

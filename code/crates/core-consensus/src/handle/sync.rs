use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_commit_certificate;
use crate::prelude::*;

pub async fn on_value_response<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    value: ValueResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.height();
    let cert_height = value.certificate.height;

    if consensus_height > cert_height {
        debug!(
            consensus.height = %consensus_height,
            certificate.height = %cert_height,
            "Received value response for lower height, ignoring"
        );
        return Ok(());
    }

    if consensus_height < cert_height {
        debug!(
            consensus.height = %consensus_height,
            certificate.height = %cert_height,
            "Received value response for higher height, queuing for later"
        );

        state.buffer_input(cert_height, Input::SyncValueResponse(value), metrics);

        return Ok(());
    }

    info!(
        certificate.height = %cert_height,
        signatures = value.certificate.commit_signatures.len(),
        "Processing value response"
    );

    let proposer = state
        .get_proposer(cert_height, value.certificate.round)
        .clone();

    let peer = value.peer;

    let effect = process_commit_certificate(co, state, metrics, value.certificate.clone())
        .await
        .map(|_| Effect::ValidSyncValue(value, proposer, Default::default()))
        .unwrap_or_else(|e| {
            error!("Error when processing commit certificate: {e}");
            Effect::InvalidSyncValue(peer, cert_height, e, Default::default())
        });

    perform!(co, effect);

    Ok(())
}

async fn process_commit_certificate<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificate: CommitCertificate<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        certificate.height = %certificate.height,
        signatures = certificate.commit_signatures.len(),
        "Processing certificate"
    );

    assert_eq!(certificate.height, state.height());

    let validator_set = state.validator_set();

    if let Err(e) = verify_commit_certificate(
        co,
        certificate.clone(),
        validator_set.clone(),
        state.params.threshold_params,
    )
    .await?
    {
        return Err(Error::InvalidCommitCertificate(certificate, e));
    }

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::CommitCertificate(certificate),
    )
    .await
}

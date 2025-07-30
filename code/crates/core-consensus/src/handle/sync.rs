use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_commit_certificate;
use crate::handle::validator_set::get_validator_set;
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
    debug!(
        value.certificate.height = %value.certificate.height,
        signatures = value.certificate.commit_signatures.len(),
        "Processing value response"
    );

    if state.driver.height() < value.certificate.height {
        debug!("Received value response for higher height, queuing for later");

        state.buffer_input(value.certificate.height, Input::SyncValueResponse(value));

        return Ok(());
    }

    if let Err(e) = process_commit_certificate(co, state, metrics, value.certificate.clone()).await
    {
        error!("Error when processing commit certificate: {e}");
        Err(e)
    } else {
        perform!(co, Effect::SyncValue(value, Default::default()));
        Ok(())
    }
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

    let Some(validator_set) = get_validator_set(co, state, certificate.height).await? else {
        return Err(Error::ValidatorSetNotFound(certificate.height));
    };

    if let Err(e) = verify_commit_certificate(
        co,
        certificate.clone(),
        validator_set.as_ref().clone(),
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

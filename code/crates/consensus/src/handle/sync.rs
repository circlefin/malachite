use std::borrow::Borrow;

use crate::handle::driver::apply_driver_input;
use crate::handle::validator_set::get_validator_set;
use crate::prelude::*;

pub async fn on_commit_certificate<Ctx>(
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
        signatures = certificate.aggregated_signature.signatures.len(),
        "Processing certificate"
    );

    let Some(validator_set) = get_validator_set(co, state, certificate.height).await? else {
        return Err(Error::ValidatorSetNotFound(certificate.height));
    };

    if let Err(e) = certificate.verify(
        &state.ctx,
        validator_set.borrow(),
        state.params.threshold_params,
    ) {
        return Err(Error::InvalidCertificate(certificate, e));
    }

    // Go to Commit step via L49
    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::CommitCertificate(certificate),
    )
    .await?;

    Ok(())
}

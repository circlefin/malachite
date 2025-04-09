use crate::handle::decide::try_decide;
use crate::handle::driver::apply_driver_input;
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

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::CommitCertificate(certificate),
    )
    .await?;

    // The CommitCertificate is provided by Value Sync, try to decide immediately, without waiting for the Commit timeout.
    // `try_decide` will check that we are in the commit step after applying the certificate to the state machine.
    try_decide(co, state, metrics).await?;

    Ok(())
}

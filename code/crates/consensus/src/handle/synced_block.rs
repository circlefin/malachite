use std::borrow::Borrow;

use bytes::Bytes;
use tracing::error;

use crate::handle::driver::apply_driver_input;
use crate::handle::validator_set::get_validator_set;
use crate::prelude::*;

pub async fn on_received_synced_block<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    block_bytes: Bytes,
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

    perform!(
        co,
        Effect::SyncedBlock {
            height: certificate.height, // TODO - should come from block
            round: certificate.round,   // TODO - should come from block
            validator_address: state.driver.address().clone(), // TODO - should come from block
            block_bytes,
        }
    );

    let Some(validator_set) = get_validator_set(co, state, certificate.height).await? else {
        // TODO: Just log an error instead?
        return Err(Error::ValidatorSetNotFound(certificate.height));
    };

    if !certificate.verify(&state.ctx, validator_set.borrow()) {
        // TODO: Return an error?
        // return Err(Error::InvalidCertificate);

        // For now, just log the error and continue
        error!(%certificate.height, %certificate.round, "Invalid certificate");

        return Ok(());
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

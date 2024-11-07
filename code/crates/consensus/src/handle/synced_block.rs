use crate::{handle::driver::apply_driver_input, prelude::*};
use bytes::Bytes;

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
            height: certificate.height,
            round: certificate.round,
            validator_address: state.driver.address().clone(),
            block_bytes,
        }
    );

    // TODO - verify aggregated signature
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

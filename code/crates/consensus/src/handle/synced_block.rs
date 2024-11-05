use crate::prelude::*;
use bytes::Bytes;

pub async fn on_received_synced_block<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
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

    // TODO - verify aggregated signature and send the majority to driver
    // on_proposal(co, state, metrics, proposal.clone()).await?;
    // on_two_third_precommits(co, state, metrics, commit).await?;

    perform!(
        co,
        Effect::SyncedBlock {
            height: certificate.height, // TODO - should come from block
            round: certificate.round,   // TODO - should come from block
            validator_address: state.driver.address().clone(), // TODO - should come from block
            block_bytes,
        }
    );

    Ok(())
}

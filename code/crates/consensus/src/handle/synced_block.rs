use crate::handle::driver::apply_driver_input;
use crate::prelude::*;
use bytes::Bytes;

pub async fn on_received_synced_block<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposal: SignedProposal<Ctx>,
    certificate: Certificate<Ctx>,
    block_bytes: Bytes,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        "Consensus processing the certificates for {}",
        proposal.height()
    );

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::Proposal(proposal.clone(), Validity::Valid),
    )
    .await?;
    for commit in certificate.commits {
        apply_driver_input(co, state, metrics, DriverInput::Vote(commit)).await?;
    }

    perform!(
        co,
        Effect::SyncedBlock {
            proposal,
            block_bytes,
        }
    );

    Ok(())
}

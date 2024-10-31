use crate::handle::proposal::on_proposal;
use crate::handle::vote::on_vote;
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
        proposal.height = %proposal.height(),
        commits = certificate.commits.len(),
        "Processing certificate"
    );

    on_proposal(co, state, metrics, proposal.clone()).await?;

    for commit in certificate.commits {
        on_vote(co, state, metrics, commit).await?;
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

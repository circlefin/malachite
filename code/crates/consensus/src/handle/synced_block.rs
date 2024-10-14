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
        "Consensus processing the certificates for {}",
        proposal.height()
    );

    on_proposal(co, state, metrics, proposal.clone()).await?;

    // TODO - debug issue with blocks that don't include commits from validators that were stopped
    // 4 vals, creates some blocks, stop val4, create blocks with 3 vals, bring val4 up, some blocks
    // that were built while away have only 2 commits (??), val4 cannot decide.
    debug!(
        "Received a certificate for {} with {} votes",
        proposal.height(),
        certificate.commits.len()
    );
    for commit in certificate.commits {
        on_vote(co, state, metrics, commit).await?;
        //apply_driver_input(co, state, metrics, DriverInput::Vote(commit)).await?;
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

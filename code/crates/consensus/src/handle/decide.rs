use crate::prelude::*;

pub async fn decide<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    consensus_round: Round,
    proposal: SignedProposal<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = proposal.height();
    let proposal_round = proposal.round();
    let value = proposal.value();

    // Clean proposals and values
    state.remove_full_proposals(height);

    // Update metrics
    {
        // We are only interested in consensus time for round 0, ie. in the happy path.
        if consensus_round == Round::new(0) {
            metrics.consensus_end();
        }

        metrics.block_end();
        metrics.finalized_blocks.inc();

        metrics
            .consensus_round
            .observe(consensus_round.as_i64() as f64);

        metrics
            .proposal_round
            .observe(proposal_round.as_i64() as f64);
    }

    #[cfg(feature = "debug")]
    {
        for trace in state.driver.get_traces() {
            debug!(%trace, "Consensus trace");
        }
    }

    // Look for an existing certificate
    let maybe_certificate: Option<&CommitCertificate<Ctx>> = state
        .driver
        .certificates
        .iter()
        .find(|&c| c.height == height && c.round == proposal_round && c.value_id == value.id());

    let certificate = match maybe_certificate {
        None => {
            // Restore the commits. Note that they will be removed from `state`
            let commits = state.restore_precommits(height, proposal_round, value);
            // TODO: should we verify we have 2/3rd commits?
            CommitCertificate::new(height, proposal_round, value.id(), commits)
        }
        Some(c) => c.clone(),
    };

    perform!(co, Effect::Decide { certificate });

    // Reinitialize to remove any previous round or equivocating precommits.
    // TODO: Revise when evidence module is added.
    state.signed_precommits.clear();

    Ok(())
}

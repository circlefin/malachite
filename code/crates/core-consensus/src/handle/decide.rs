use crate::prelude::*;

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
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

    // We only decide proposals for the current height
    assert_eq!(height, state.driver.height());

    // Clean proposals and values
    state.remove_full_proposals(height);

    // Update metrics
    #[cfg(feature = "metrics")]
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
    let certificate = state
        .driver
        .get_certificate(proposal_round, value.id())
        .cloned()
        .unwrap_or_else(|| {
            // Restore the commits. Note that they will be removed from `state`
            let commits = state.restore_precommits(height, proposal_round, value);
            // TODO: should we verify we have 2/3rd commits?
            CommitCertificate::new(height, proposal_round, value.id(), commits)
        });

    perform!(co, Effect::Decide(certificate, Default::default()));

    // Reinitialize to remove any previous round or equivocating precommits.
    // TODO: Revise when evidence module is added.
    state.signed_precommits.clear();

    Ok(())
}

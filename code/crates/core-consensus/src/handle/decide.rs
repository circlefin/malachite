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
    let (certificate, extensions) = state
        .driver
        .commit_certificate(proposal_round, value.id())
        .cloned()
        .map(|certificate| (certificate, VoteExtensions::default()))
        .unwrap_or_else(|| {
            // Restore the commits. Note that they will be removed from `state`
            let mut commits = state.restore_precommits(height, proposal_round, value);

            let extensions = extract_vote_extensions(&mut commits);

            // TODO: Should we verify we have 2/3rd commits?
            let certificate = CommitCertificate::new(height, proposal_round, value.id(), commits);

            (certificate, extensions)
        });

    perform!(
        co,
        Effect::Decide(certificate, extensions, Default::default())
    );

    // Reinitialize to remove any previous round or equivocating precommits.
    // TODO: Revise when evidence module is added.
    state.signed_precommits.clear();

    Ok(())
}

// Extract vote extensions from a list of votes,
// removing them from each vote in the process.
pub fn extract_vote_extensions<Ctx: Context>(votes: &mut [SignedVote<Ctx>]) -> VoteExtensions<Ctx> {
    let extensions = votes
        .iter_mut()
        .filter_map(|vote| {
            vote.message
                .take_extension()
                .map(|e| (vote.validator_address().clone(), e))
        })
        .collect();

    VoteExtensions::new(extensions)
}

/// Decide on the current proposal without waiting for Commit timeout.
pub async fn decide_current_no_timeout<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = state.driver.height();
    let round = state.driver.round();

    perform!(
        co,
        Effect::CancelTimeout(Timeout::commit(round), Default::default())
    );

    let proposal = state
        .decision
        .remove(&(height, round))
        .ok_or_else(|| Error::DecidedValueNotFound(height, round))?;

    decide(co, state, metrics, round, proposal).await
}

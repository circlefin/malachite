use crate::{handle::signature::verify_certificate, prelude::*};

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn try_decide<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if !state.driver.step_is_commit() {
        return Ok(());
    }

    let height = state.driver.height();
    let consensus_round = state.driver.round();

    let Some((proposal_round, decided_value)) = state.decided_value() else {
        return Err(Error::DecisionNotFound(height, consensus_round));
    };

    let decided_id = decided_value.id();

    // Look for an existing certificate
    let (certificate, extensions) = state
        .driver
        .commit_certificate(proposal_round, decided_id.clone())
        .cloned()
        .map(|certificate| (certificate, VoteExtensions::default()))
        .unwrap_or_else(|| {
            // Restore the commits. Note that they will be removed from `state`
            let mut commits = state.restore_precommits(height, proposal_round, &decided_value);

            let extensions = extract_vote_extensions(&mut commits);

            let certificate =
                CommitCertificate::new(height, proposal_round, decided_id.clone(), commits);

            (certificate, extensions)
        });

    let Some((proposal, _)) = state.driver.proposal_and_validity_for_round(proposal_round) else {
        return Err(Error::DriverProposalNotFound(height, proposal_round));
    };

    let Some(full_proposal) =
        state.full_proposal_at_round_and_value(&height, proposal_round, &decided_value)
    else {
        return Err(Error::FullProposalNotFound(height, proposal_round));
    };

    if proposal.value().id() != decided_id {
        info!(
            "Decide: driver proposal value id {} does not match the decided value id {}, this may happen if consensus and value sync run in parallel",
            proposal.value().id(),
            decided_id
        );
    }

    assert_eq!(full_proposal.builder_value.id(), decided_id);
    assert_eq!(full_proposal.proposal.value().id(), decided_id);
    assert_eq!(full_proposal.validity, Validity::Valid);

    // The certificate must be valid if state is Commit
    assert!(verify_certificate(
        co,
        certificate.clone(),
        state.driver.validator_set().clone(),
        state.params.threshold_params,
    )
    .await
    .is_ok());

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

    perform!(
        co,
        Effect::CancelTimeout(Timeout::commit(state.driver.round()), Default::default())
    );

    if !state.decided_sent {
        state.decided_sent = true;

        perform!(
            co,
            Effect::Decide(certificate, extensions, Default::default())
        );
    }

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

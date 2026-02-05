use crate::handle::signature::verify_commit_certificate;
use crate::prelude::*;
use crate::MisbehaviorEvidence;

#[cfg_attr(not(feature = "metrics"), allow(unused_variables))]
pub async fn decide<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    assert!(state.driver.step_is_commit());

    let height = state.height();
    let consensus_round = state.round();

    let Some((proposal_round, decided_value)) = state.decided_value() else {
        return Err(Error::DecisionNotFound(height, consensus_round));
    };

    let decided_id = decided_value.id();

    // Look for an existing certificate (from sync) or build one from precommits
    let existing_certificate = state
        .driver
        .commit_certificate(proposal_round, &decided_id)
        .cloned();

    // FIXME: there is actual no guarantee that associated vote extensions can be found,
    // in particular when deciding via sync, see: https://circlepay.atlassian.net/browse/CCHAIN-915.
    let (certificate, extensions) = existing_certificate
        .map(|certificate| (certificate, VoteExtensions::default()))
        .unwrap_or_else(|| {
            // Restore the commits. Note that they will be removed from `state`
            let mut commits = state.restore_precommits(height, proposal_round, &decided_value);

            let extensions = extract_vote_extensions(&mut commits);

            let certificate =
                CommitCertificate::new(height, proposal_round, decided_id.clone(), commits);

            (certificate, extensions)
        });

    // The certificate must be valid in Commit step
    assert!(
        verify_commit_certificate(
            co,
            certificate.clone(),
            state.driver.validator_set().clone(),
            state.params.threshold_params,
        )
        .await?
        .is_ok(),
        "Decide: Commit certificate is not valid"
    );

    // Update metrics
    #[cfg(feature = "metrics")]
    {
        // We are only interested in consensus time for round 0, ie. in the happy path.
        if consensus_round == Round::new(0) {
            metrics.consensus_end();
        }

        metrics.block_end();

        metrics
            .consensus_round
            .observe(consensus_round.as_i64() as f64);

        metrics
            .proposal_round
            .observe(proposal_round.as_i64() as f64);
    }

    let evidence = MisbehaviorEvidence {
        proposals: state.driver.take_proposal_evidence(),
        votes: state.driver.take_vote_evidence(),
    };

    perform!(
        co,
        Effect::Decide(
            certificate.clone(),
            extensions.clone(),
            evidence,
            Default::default()
        )
    );

    let Some(target_time) = state.target_time else {
        debug!(
            height = %height,
            "No target time set, finalizing immediately"
        );

        super::finalize::log_and_finalize(co, state, certificate, extensions).await?;
        return Ok(());
    };

    let start_time = state
        .height_start_time
        .expect("height_start_time must be set when target_time is set");
    let elapsed = start_time.elapsed();

    if elapsed < target_time {
        state.finalization_period = true;

        let remaining = target_time - elapsed;
        let timeout = Timeout::finalize_height(consensus_round, remaining);
        perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));

        debug!(
            height = %height,
            remaining_ms = remaining.as_millis(),
            "Entering finalization period"
        );
    } else {
        debug!(
            height = %height,
            elapsed_ms = elapsed.as_millis(),
            target_ms = target_time.as_millis(),
            "Target time exceeded, finalizing immediately"
        );

        super::finalize::log_and_finalize(co, state, certificate, extensions).await?;
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

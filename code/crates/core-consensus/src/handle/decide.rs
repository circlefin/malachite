use std::time::Duration;

use crate::handle::signature::verify_commit_certificate;
use crate::prelude::*;

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

    // Look for an existing certificate in the driver. This may be present if the decision was reached via Sync protocol.
    let existing_certificate = state
        .driver
        .commit_certificate(proposal_round, &decided_id)
        .cloned();

    // Determine if we have an existing certificate or need to restore one.
    let (certificate, extensions) = if let Some(certificate) = existing_certificate {
        // NOTE: Existence implies the decision was reached via Sync protocol.
        // FIXME: No guarantee vote extensions are found in sync. (CCHAIN-915)
        (certificate, VoteExtensions::default())
    } else {
        // Restore the precommits (removes them from `state`).
        let mut commits = state.restore_precommits(height, proposal_round, &decided_value);
        let extensions = extract_vote_extensions(&mut commits);
        let certificate = CommitCertificate::new(height, proposal_round, decided_id, commits);
        (certificate, extensions)
    };

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

    perform!(
        co,
        Effect::Decide(certificate.clone(), extensions.clone(), Default::default())
    );

    // Calculate remaining time until target (0 if no target or already exceeded)
    let remaining = state
        .target_time
        .and_then(|target| {
            let elapsed = state
                .height_start_time
                .expect("height_start_time must be set when target_time is set")
                .elapsed();
            target.checked_sub(elapsed)
        })
        .unwrap_or(Duration::ZERO);

    // Enter finalization period
    debug!(%height, ?remaining, "Entering finalization period");
    state.finalization_period = true;

    let timeout = Timeout::finalize_height(consensus_round, remaining);
    perform!(co, Effect::ScheduleTimeout(timeout, Default::default()));

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

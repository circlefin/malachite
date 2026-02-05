use crate::{handle::signature::verify_commit_certificate, prelude::*, MisbehaviorEvidence};

pub async fn finalize_height<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    assert!(
        state.driver.step_is_commit(),
        "finalize_height can only be called in Commit step"
    );
    assert!(
        state.finalization_period,
        "finalize_height can only be called during finalization period"
    );

    let height = state.height();

    let Some((proposal_round, decided_value)) = state.decided_value() else {
        return Err(Error::DecisionNotFound(height, state.round()));
    };

    let decided_id = decided_value.id();

    let mut commits = state.restore_precommits(height, proposal_round, &decided_value);
    let extensions = super::decide::extract_vote_extensions(&mut commits);
    let certificate = CommitCertificate::new(height, proposal_round, decided_id, commits);

    assert!(
        verify_commit_certificate(
            co,
            certificate.clone(),
            state.driver.validator_set().clone(),
            state.params.threshold_params,
        )
        .await?
        .is_ok(),
        "Finalize: Commit certificate is not valid"
    );

    state.finalization_period = false;

    log_and_finalize(co, state, certificate, extensions).await?;

    Ok(())
}

/// Emit the Finalize effect with a pre-built certificate and extensions.
pub async fn log_and_finalize<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    certificate: CommitCertificate<Ctx>,
    extensions: VoteExtensions<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        height = %certificate.height,
        round = %certificate.round,
        "Finalizing height"
    );

    #[cfg(feature = "debug")]
    {
        for trace in state.driver.get_traces() {
            debug!(%trace, "Finalize: Consensus trace");
        }
    }

    let evidence = MisbehaviorEvidence {
        proposals: state.driver.take_proposal_evidence(),
        votes: state.driver.take_vote_evidence(),
    };

    perform!(
        co,
        Effect::Finalize(certificate, extensions, evidence, Default::default())
    );

    Ok(())
}

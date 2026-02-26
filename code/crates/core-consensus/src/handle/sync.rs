use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_commit_certificate;
use crate::prelude::*;
use crate::types::ProposedValue;

/// Tries to decide on a value using the direct SyncDecision path.
///
/// - if the value is Valid, we decide using the `SyncDecision` path
/// - if the value is Invalid, `SyncDecision` path is not possible and an error is logged
///
/// # Preconditions
///
/// A proposed value and a commit certificate for that value must already be stored.
/// The caller detects that preconditions are met:
///
/// * `on_proposed_value()` - We receive a proposed value and a commit certificate is already present
///   * Sync origin: typical case, certificate processed first, then proposed value
///   * Consensus origin: atypical, race between sync and consensus with WAL-ed messages
///
/// * `process_commit_certificate()` - We receive a commit certificate for an already-stored value
///   * Consensus origin: typical case on restart with proposed value in WAL, recovering via sync
///   * Sync origin: atypical, or if engine changes processing order
pub async fn maybe_sync_decision<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposed_value: ProposedValue<Ctx>,
    origin: ValueOrigin,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let height = proposed_value.height;
    let round = proposed_value.round;
    let value = proposed_value.value;
    let proposer = proposed_value.proposer;

    if proposed_value.validity.is_invalid() {
        // We have a certificate indicating that > 2/3 of the validators have committed to value_id.
        // Which means that > 2/3rds of the validators have classified the value as Valid,
        // while local application has classified it as Invalid, in current or previous instance.
        // If Invalid comes from a previous instance (WAL replay), we recover when the sync proposed value is processed.
        // Otherwise, consensus will be blocked until application restarts with correct version.
        error!(
            %height, %round, value_id = %value.id(),
            "Commit certificate for invalid value from {origin:?}"
        );

        return Ok(());
    }

    debug!(%height, %round, value_id = %value.id(), "Using sync decision path");

    // The consensus implementation requires a Proposal for deciding a value.
    // Produce a synthetic proposal to be processed by the consensus state-machine.
    // POL round is Nil since it's not needed for the decision.
    let proposal = Ctx::new_proposal(&state.ctx, height, round, value, Round::Nil, proposer);

    // The decide step will detect this is a SyncDecision path, from the existence
    // of the Commit certificate, and skip the requirement for a signed proposal.
    apply_driver_input(co, state, metrics, DriverInput::SyncDecision(proposal)).await
}

pub async fn on_value_response<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    value: ValueResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.height();
    let cert_height = value.certificate.height;

    if consensus_height > cert_height {
        debug!(
            consensus.height = %consensus_height,
            certificate.height = %cert_height,
            "Received value response for lower height, ignoring"
        );
        return Ok(());
    }

    if consensus_height < cert_height {
        warn!(
            consensus.height = %consensus_height,
            certificate.height = %cert_height,
            "Received sync value response for higher height; should have been buffered by sync actor"
        );

        return Ok(());
    }

    info!(
        certificate.height = %cert_height,
        signatures = value.certificate.commit_signatures.len(),
        "Processing value response"
    );

    let proposer = state
        .get_proposer(cert_height, value.certificate.round)
        .clone();

    let peer = value.peer;

    let effect = process_commit_certificate(co, state, metrics, value.certificate.clone())
        .await
        .map(|_| Effect::ValidSyncValue(value, proposer, Default::default()))
        .unwrap_or_else(|e| {
            error!("Error when processing commit certificate: {e}");
            Effect::InvalidSyncValue(peer, cert_height, e, Default::default())
        });

    perform!(co, effect);

    Ok(())
}

async fn process_commit_certificate<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificate: CommitCertificate<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        certificate.height = %certificate.height,
        signatures = certificate.commit_signatures.len(),
        "Processing certificate"
    );

    assert_eq!(certificate.height, state.height());

    let validator_set = state.validator_set();

    if let Err(e) = verify_commit_certificate(
        co,
        certificate.clone(),
        validator_set.clone(),
        state.params.threshold_params,
    )
    .await?
    {
        return Err(Error::InvalidCommitCertificate(certificate, e));
    }

    // Extract fields before moving certificate
    let cert_height = certificate.height;
    let cert_round = certificate.round;
    let cert_value_id = certificate.value_id.clone();

    // Store the commit certificate in the driver
    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::CommitCertificate(certificate),
    )
    .await?;

    // If we have received a ProposedValue input matching the certified value try the sync decision path.
    if let Some(proposed_value) =
        state.get_proposed_value_by_id(cert_height, cert_round, &cert_value_id)
    {
        // Try the sync decision path from consensus origin (WAL replayed value or consensus value during sync race)
        maybe_sync_decision(
            co,
            state,
            metrics,
            proposed_value.clone(),
            ValueOrigin::Consensus,
        )
        .await?;
    }

    Ok(())
}

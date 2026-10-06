use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::state::ProposedValueStorage;
use crate::types::ProposedValue;

use super::sync::maybe_sync_decision;

/// Handles a proposed value that is not originated from the sync protocol.
///
/// This method looks for a matching (valid and signed) Proposal message to produce a
/// Proposal driver's input, and applies it to the driver.
///
/// Stores the value and applies any associated proposals to the driver.
async fn process_proposal<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposed_value: ProposedValue<Ctx>,
    validity: Validity,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // Get all proposals we have for this value.
    let proposals = state.proposals_for_value(&proposed_value);

    // Apply all proposals we have for this value, with the stored validity.
    for signed_proposal in proposals {
        debug!(
            proposal.height = %signed_proposal.height(),
            proposal.round = %signed_proposal.round(),
            validity = ?validity,
            "We have a full proposal for this round, checking..."
        );

        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::Proposal(signed_proposal, validity),
        )
        .await?;
    }

    Ok(())
}

/// Handles a proposed value that can originate from multiple sources:
/// 1. Application layer, in 'proposal-and-parts' mode
/// 2. WAL (Write-Ahead Log), replayed during node recovery
/// 3. Sync protocol as part of state synchronization
///
/// This function processes proposed values based on their height:
/// - Drops values from lower heights
/// - Queues values from higher heights for later processing
/// - If a commit certificate exists for this value, uses direct decision path
/// - Stores the value and appends it to the WAL if new
/// - Applies any associated proposals to the driver
///
/// # Arguments
/// * `co` - Coordination object for async operations
/// * `state` - Current consensus state
/// * `metrics` - Metrics collection
/// * `proposed_value` - The proposed value to process
/// * `origin` - Origin of the proposed value (e.g., Sync, Network)
///
/// # Returns
/// Result indicating success or failure of processing the proposed value
pub async fn on_proposed_value<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    mut proposed_value: ProposedValue<Ctx>,
    origin: ValueOrigin,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.height() > proposed_value.height {
        debug!("Received value for lower height, dropping");
        return Ok(());
    }

    if state.height() < proposed_value.height {
        debug!(
            consensus.height = %state.height(),
            value.height = %proposed_value.height,
            "Received value for higher height, queuing for later"
        );

        state.buffer_input(
            proposed_value.height,
            Input::ProposedValue(proposed_value, origin),
            metrics,
        );

        return Ok(());
    }

    // Do not apply the future-round lookahead here. WAL decode always reconstructs ProposedValue
    // as ValueOrigin::Consensus and does not persist commit certificates, so an origin- or
    // certificate-gated bound would drop a crash-recovered sync value. Gossip flood is closed on
    // Proposal messages in on_proposal.

    // Select a bounded keeper placement before persisting the value. Sync values retain their
    // source round. A consensus value can instead update matching entries without growing its
    // full source bucket, or be retained at a round where it holds a polka certificate.
    let value_id = proposed_value.value.id();
    let storage = if origin.is_consensus() {
        match state.proposed_value_storage(proposed_value.height, proposed_value.round, &value_id) {
            Ok(storage) => storage,
            Err(error) => {
                warn!(
                    consensus.height = %state.height(),
                    value.height = %proposed_value.height,
                    value.round = %proposed_value.round,
                    value.id = ?value_id,
                    reason = %error,
                    "Rejecting proposed value"
                );
                #[cfg(feature = "metrics")]
                metrics.dropped_capped_proposed_values.inc();

                return Ok(());
            }
        }
    } else {
        ProposedValueStorage::SourceRoundAndMatchingEntries
    };

    // We may consider in the future some optimization to avoid multiple identical entries in the
    // WAL, in the case of multiple node restarts. For now we write every ProposedValue to it.
    perform!(
        co,
        Effect::WalAppend(
            proposed_value.height,
            Input::ProposedValue(proposed_value.clone(), origin),
            Default::default()
        )
    );

    // We MUST stick to the stored validity, which may have been updated
    // when storing the value (e.g., from Invalid to Valid).
    let validity = state.store_value_with_storage(&proposed_value, storage);
    proposed_value.validity = validity;

    let certificate_available = state
        .driver
        .commit_certificate(proposed_value.round, &value_id)
        .is_some();

    if certificate_available {
        // We have a proposed value and its Commit certificate, we try to decide using the sync decision path.
        maybe_sync_decision(co, state, metrics, proposed_value, origin).await
    } else {
        // We have a proposed value but no Commit certificate, we need to find a Proposal for it.
        process_proposal(co, state, metrics, proposed_value, validity).await
    }
}

use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::{ProposedValue, WalEntry};

use super::signature::sign_proposal;

/// Handles a proposed value that can originate from multiple sources:
/// 1. Application layer:
///    - In 'parts-only' mode
///    - In 'proposal-and-parts' mode
/// 2. WAL (Write-Ahead Log) during node restart recovery
/// 3. Sync service during state synchronization
///
/// This function processes proposed values based on their height and origin:
/// - Drops values from lower heights
/// - Queues values from higher heights for later processing
/// - For parts-only mode or values from Sync, generates and signs internal Proposal messages
/// - Stores the value and appends it to the WAL if new
/// - Applies any associated proposals to the driver
/// - Attempts immediate decision for values from Sync
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
    proposed_value: ProposedValue<Ctx>,
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

    // There are two cases where we need to generate an internal Proposal message for consensus to process the full proposal:
    // a) In parts-only mode, where we do not get a Proposal message but only the proposal parts
    // b) In any mode if the proposed value was provided by Sync, where we do net get a Proposal message but only the full value and the certificate
    if state.params.value_payload.parts_only() || origin.is_sync() {
        let proposal = Ctx::new_proposal(
            &state.ctx,
            proposed_value.height,
            proposed_value.round,
            proposed_value.value.clone(),
            proposed_value.valid_round,
            proposed_value.proposer.clone(),
        );

        // TODO: Keep unsigned proposals in keeper.
        // For now we keep all happy by signing all "implicit" proposals with this node's key
        let signed_proposal = sign_proposal(co, proposal).await?;

        state.store_proposal(signed_proposal);
    }

    // If this is the first time we see this value, append it to the WAL, so it can be used for recovery.
    if !state.value_exists(&proposed_value) {
        perform!(
            co,
            Effect::WalAppend(
                proposed_value.height,
                WalEntry::ProposedValue(proposed_value.clone()),
                Default::default()
            )
        );
    }

    state.store_value(&proposed_value);

    let validity = proposed_value.validity;
    let proposals = state.proposals_for_value(&proposed_value);

    for signed_proposal in proposals {
        debug!(
            proposal.height = %signed_proposal.height(),
            proposal.round = %signed_proposal.round(),
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

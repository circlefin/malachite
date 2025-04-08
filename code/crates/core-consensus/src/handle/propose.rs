use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::{LocallyProposedValue, ProposedValue, WalEntry};

/// Handles a locally proposed value.
/// Called when the application has built a value to propose.
///
/// This function processes a value proposed by the local node:
/// - Validates that the height and round match the current state
/// - Creates a ProposedValue with the local node as proposer
/// - Appends the value to the WAL if it hasn't been seen before
/// - Stores the value in the state
/// - Applies the proposal to the driver
///
/// # Arguments
/// * `co` - Coordination object for handling effects
/// * `state` - Current consensus state
/// * `metrics` - Metrics collection object
/// * `local_value` - The value being proposed locally
///
/// # Returns
/// `Result<(), Error<Ctx>>` - Ok if the proposal was processed successfully
pub async fn on_propose<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    local_value: LocallyProposedValue<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.driver.height() != local_value.height {
        warn!(
            "Ignoring value for height {}, current height: {}",
            local_value.height,
            state.driver.height()
        );

        return Ok(());
    }

    if state.driver.round() != local_value.round {
        warn!(
            "Ignoring value for round {}, current round: {}",
            local_value.round,
            state.driver.round()
        );

        return Ok(());
    }

    let proposed_value = ProposedValue {
        height: local_value.height,
        round: local_value.round,
        valid_round: Round::Nil,
        proposer: state.address().clone(),
        value: local_value.value.clone(),
        validity: Validity::Valid,
    };

    #[cfg(feature = "metrics")]
    metrics.consensus_start();

    // If this is the first time we see this value in the current round, append it to the WAL
    if !state.value_exists(&proposed_value) {
        perform!(
            co,
            Effect::WalAppend(
                WalEntry::ProposedValue(proposed_value.clone()),
                Default::default()
            )
        );
    }

    state.store_value(&proposed_value);

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::ProposeValue(local_value.round, local_value.value),
    )
    .await
}

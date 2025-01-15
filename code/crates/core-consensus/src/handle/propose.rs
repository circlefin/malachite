use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::{ProposedValue, ValueToPropose};

pub async fn on_propose<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    value: ValueToPropose<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let ValueToPropose {
        height,
        round,
        valid_round,
        value,
        extension,
    } = value;

    if state.driver.height() != height {
        warn!(
            "Ignoring proposal for height {height}, current height: {}",
            state.driver.height()
        );

        return Ok(());
    }

    if state.driver.round() != round {
        warn!(
            "Ignoring propose value for round {round}, current round: {}",
            state.driver.round()
        );

        return Ok(());
    }
    #[cfg(feature = "metrics")]
    metrics.consensus_start();

    state.store_value(&ProposedValue {
        height,
        round,
        valid_round,
        proposer: state.address().clone(),
        value: value.clone(),
        validity: Validity::Valid,
        extension,
    });

    apply_driver_input(co, state, metrics, DriverInput::ProposeValue(round, value)).await
}

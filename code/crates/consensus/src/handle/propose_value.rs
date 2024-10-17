use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::ProposedValue;

pub async fn propose_value<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    round: Round,
    value: Ctx::Value,
    extension: Extension,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
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

    metrics.consensus_start();

    state.store_value(&ProposedValue {
        height,
        round,
        validator_address: state.driver.address().clone(),
        value: value.clone(),
        validity: Validity::Valid,
        extension,
    });

    apply_driver_input(co, state, metrics, DriverInput::ProposeValue(round, value)).await
}

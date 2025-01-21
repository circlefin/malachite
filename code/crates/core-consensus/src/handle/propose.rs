use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::{LocallyProposedValue, ProposedValue};

pub async fn on_propose<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    value: LocallyProposedValue<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let LocallyProposedValue {
        height,
        round,
        value,
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
        valid_round: Round::Nil,
        proposer: state.address().clone(),
        value: value.clone(),
        validity: Validity::Valid,
    });

    apply_driver_input(co, state, metrics, DriverInput::ProposeValue(round, value)).await
}

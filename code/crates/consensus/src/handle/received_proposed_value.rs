use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::ProposedValue;

#[tracing::instrument(
    skip_all,
    fields(
        height = %proposed_value.height,
        round = %proposed_value.round,
        validity = ?proposed_value.validity,
        id = %proposed_value.value.id()
    )
)]
pub async fn on_received_proposed_value<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    proposed_value: ProposedValue<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.driver.height() > proposed_value.height {
        debug!("Received value for lower height, dropping");
        return Ok(());
    }

    if state.driver.height() < proposed_value.height {
        if state.driver.height().increment() == proposed_value.height {
            debug!("Received value for next height, queuing for later");
            state
                .input_queue
                .push_back(Input::ReceivedProposedValue(proposed_value));
        }
        return Ok(());
    }

    state.store_value(&proposed_value);

    let proposals = state.full_proposals_for_value(&proposed_value);
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
            DriverInput::Proposal(signed_proposal, proposed_value.validity),
        )
        .await?;
    }

    Ok(())
}

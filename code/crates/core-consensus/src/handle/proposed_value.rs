use crate::prelude::*;

use crate::handle::driver::apply_driver_input;
use crate::types::ProposedValue;

use super::decide::try_decide;
use super::signature::sign_proposal;

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
    if state.driver.height() > proposed_value.height {
        debug!("Received value for lower height, dropping");
        return Ok(());
    }

    if state.driver.height() < proposed_value.height {
        debug!("Received value for higher height, queuing for later");

        state.buffer_input(
            proposed_value.height,
            Input::ProposedValue(proposed_value, origin),
        );

        return Ok(());
    }

    state.store_value(&proposed_value);

    // There are two cases where we need to generate an internal Proposal message for consensus to process the full proposal:
    // a) In parts-only mode, where we do not get a Proposal message but only the proposal parts
    // b) In any mode if the proposed value was provided by Sync, where we do net get a Proposal message but only the full value and the certificate
    if state.params.value_payload.parts_only() || origin == ValueOrigin::Sync {
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

    if origin == ValueOrigin::Sync {
        // The proposed value was provided by Sync, try to decide immediately, without waiting for the Commit timeout.
        // `try_decide` will check that we are in the commit step after applying the proposed value to the state machine.
        try_decide(co, state, metrics).await?;
    }

    Ok(())
}

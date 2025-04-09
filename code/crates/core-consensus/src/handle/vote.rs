use tracing::trace;

use crate::handle::driver::apply_driver_input;
use crate::input::Input;
use crate::prelude::*;
use crate::types::{SignedConsensusMsg, WalEntry};
use crate::util::pretty::PrettyVote;

pub async fn on_vote<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    signed_vote: SignedVote<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.driver.height();
    let consensus_round = state.driver.round();
    let vote_height = signed_vote.height();
    let validator_address = signed_vote.validator_address();

    if consensus_height > vote_height {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote for lower height, dropping"
        );

        return Ok(());
    }

    info!(
        height = %consensus_height,
        %vote_height,
        address = %validator_address,
        message = %PrettyVote::<Ctx>(&signed_vote.message),
        "Received vote",
    );

    // Queue messages if driver is not initialized, or if they are for higher height.
    // Process messages received for the current height.
    // Drop all others.
    if consensus_round == Round::Nil {
        trace!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote at round -1, queuing for later"
        );

        state.buffer_input(vote_height, Input::Vote(signed_vote));

        return Ok(());
    }

    if consensus_height < vote_height {
        trace!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote for higher height, queuing for later"
        );

        state.buffer_input(vote_height, Input::Vote(signed_vote));

        return Ok(());
    }

    debug_assert_eq!(consensus_height, vote_height);

    // Only append to WAL and store the non-nil precommit if we have not yet seen this vote.
    if !state.driver.votes().has_vote(&signed_vote) {
        // Append the vote to the Write-ahead Log
        perform!(
            co,
            Effect::WalAppend(
                WalEntry::ConsensusMsg(SignedConsensusMsg::Vote(signed_vote.clone())),
                Default::default()
            )
        );
    }

    apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote)).await?;

    Ok(())
}

use tracing::trace;

use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_signature;
use crate::handle::validator_set::get_validator_set;
use crate::input::Input;
use crate::prelude::*;
use crate::types::{ConsensusMsg, SignedConsensusMsg, WalEntry};
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

    if !verify_signed_vote(co, state, &signed_vote).await? {
        return Ok(());
    }

    info!(
        height = %consensus_height,
        %vote_height,
        address = %validator_address,
        message = %PrettyVote::<Ctx>(&signed_vote.message),
        "Received vote",
    );

    // Only process this vote if we have not yet seen it.
    if !state.driver.votes().has_vote(&signed_vote) {
        perform!(
            co,
            Effect::WalAppend(
                WalEntry::ConsensusMsg(SignedConsensusMsg::Vote(signed_vote.clone())),
                Default::default()
            )
        );

        apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote)).await?;
    }

    Ok(())
}

pub async fn verify_signed_vote<Ctx>(
    co: &Co<Ctx>,
    state: &State<Ctx>,
    signed_vote: &SignedVote<Ctx>,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.driver.height();
    let vote_height = signed_vote.height();
    let vote_round = signed_vote.round();
    let validator_address = signed_vote.validator_address();

    let Some(validator_set) = get_validator_set(co, state, signed_vote.height()).await? else {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            vote.round = %vote_round,
            validator = %validator_address,
            "Received vote for height without known validator set, dropping"
        );

        return Ok(false);
    };

    let Some(validator) = validator_set.get_by_address(validator_address) else {
        warn!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            vote.round = %vote_round,
            validator = %validator_address,
            "Received vote from unknown validator"
        );

        return Ok(false);
    };

    let signed_msg = signed_vote.clone().map(ConsensusMsg::Vote);
    if !verify_signature(co, signed_msg, validator).await? {
        warn!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            vote.round = %vote_round,
            validator = %validator_address,
            "Received vote with invalid signature: {}", PrettyVote::<Ctx>(&signed_vote.message)
        );

        return Ok(false);
    }

    verify_vote_extension(co, state, signed_vote, validator).await
}

async fn verify_vote_extension<Ctx>(
    co: &Co<Ctx>,
    state: &State<Ctx>,
    vote: &SignedVote<Ctx>,
    validator: &Ctx::Validator,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    let VoteType::Precommit = vote.vote_type() else {
        return Ok(true);
    };

    let NilOrVal::Val(value_id) = vote.value().as_ref() else {
        return Ok(true);
    };

    let Some(extension) = vote.extension() else {
        return Ok(true);
    };

    let result = perform!(
        co,
        Effect::VerifyVoteExtension(
            vote.height(),
            vote.round(),
            value_id.clone(),
            extension.clone(),
            validator.public_key().clone(),
            Default::default()
        ),
        Resume::VoteExtensionValidity(result) => result
    );

    if let Err(e) = result {
        warn!(
            consensus.height = %state.driver.height(),
            vote.height = %vote.height(),
            vote.round = %vote.round(),
            validator = %validator.address(),
            "Received vote with invalid extension: {}, reason: {e}",
            PrettyVote::<Ctx>(&vote.message)
        );

        return Ok(false);
    }

    Ok(true)
}

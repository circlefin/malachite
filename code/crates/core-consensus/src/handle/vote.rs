use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_signature;
use crate::input::Input;
use crate::params::MAX_FUTURE_ROUND_LOOKAHEAD;
use crate::prelude::*;
use crate::types::ConsensusMsg;
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
    let consensus_height = state.height();
    let consensus_round = state.round();
    let vote_height = signed_vote.height();
    let vote_round = signed_vote.round();
    let validator_address = signed_vote.validator_address();

    // Discard votes for heights lower than the current height.
    if consensus_height > vote_height {
        debug!(
            consensus.height = %consensus_height,
            consensus.round = %consensus_round,
            vote.height = %vote_height,
            vote.round = %vote_round,
            vote.msg = %PrettyVote::<Ctx>(&signed_vote.message),
            validator = %validator_address,
            "Received vote for lower height, dropping"
        );

        return Ok(());
    }

    // Queue votes for heights higher than the current height.
    if consensus_height < vote_height {
        debug!(
            consensus.height = %consensus_height,
            consensus.round = %consensus_round,
            vote.height = %vote_height,
            vote.round = %vote_round,
            vote.msg = %PrettyVote::<Ctx>(&signed_vote.message),
            validator = %validator_address,
            "Received vote for higher height, queuing for later"
        );

        state.buffer_input(vote_height, Input::Vote(signed_vote), metrics);

        return Ok(());
    }

    // Queue messages if driver is not initialized
    // Process messages received for the current height.
    // Drop all others.
    if consensus_round == Round::Nil {
        debug!(
            consensus.height = %consensus_height,
            consensus.round = %consensus_round,
            vote.height = %vote_height,
            vote.round = %vote_round,
            vote.msg = %PrettyVote::<Ctx>(&signed_vote.message),
            validator = %validator_address,
            "Received vote at round -1, queuing for later"
        );

        state.buffer_input(vote_height, Input::Vote(signed_vote), metrics);

        return Ok(());
    }

    debug_assert_eq!(consensus_height, vote_height);

    // Drop votes whose round is too far ahead of the current consensus round.
    // This bounds per-height vote-keeper state, signature verification work,
    // and WAL I/O when votes carry arbitrarily high round numbers.
    let ceiling = consensus_round
        .as_i64()
        .saturating_add(i64::from(MAX_FUTURE_ROUND_LOOKAHEAD));
    if vote_round.as_i64() > ceiling {
        debug!(
            consensus.height = %consensus_height,
            consensus.round = %consensus_round,
            vote.height = %vote_height,
            vote.round = %vote_round,
            validator = %validator_address,
            "Received vote for round beyond the future-round lookahead, dropping"
        );

        #[cfg(feature = "metrics")]
        metrics.dropped_future_round_votes.inc();

        return Ok(());
    }

    // Only process this vote if we have not yet seen it.
    if state.driver.votes().has_vote(&signed_vote) {
        return Ok(());
    }

    if !verify_signed_vote(co, state, &signed_vote).await? {
        return Ok(());
    }

    info!(
        consensus.height = %consensus_height,
        consensus.round = %consensus_round,
        vote.height = %vote_height,
        vote.round = %vote_round,
        vote.msg = %PrettyVote::<Ctx>(&signed_vote.message),
        validator = %validator_address,
        "Received vote",
    );

    perform!(
        co,
        Effect::WalAppend(
            signed_vote.height(),
            Input::Vote(signed_vote.clone()),
            Default::default()
        )
    );

    apply_driver_input(co, state, metrics, DriverInput::Vote(signed_vote)).await?;

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
    let consensus_height = state.height();
    let vote_height = signed_vote.height();
    let vote_round = signed_vote.round();
    let validator_address = signed_vote.validator_address();

    assert_eq!(vote_height, consensus_height);

    let validator_set = state.validator_set();

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
        if vote.extension().is_some() {
            warn!(
                consensus.height = %state.height(),
                vote.height = %vote.height(),
                vote.round = %vote.round(),
                validator = %validator.address(),
                "Received non-precommit vote with vote extension: {}",
                PrettyVote::<Ctx>(&vote.message)
            );

            return Ok(false);
        }

        return Ok(true);
    };

    let NilOrVal::Val(value_id) = vote.value().as_ref() else {
        if vote.extension().is_some() {
            warn!(
                consensus.height = %state.height(),
                vote.height = %vote.height(),
                vote.round = %vote.round(),
                validator = %validator.address(),
                "Received nil precommit with vote extension: {}",
                PrettyVote::<Ctx>(&vote.message)
            );

            return Ok(false);
        }

        return Ok(true);
    };

    let Some(extension) = vote.extension() else {
        if state.vote_extension_policy.is_required() {
            warn!(
                consensus.height = %state.height(),
                vote.height = %vote.height(),
                vote.round = %vote.round(),
                validator = %validator.address(),
                "Received non-nil precommit without required vote extension: {}",
                PrettyVote::<Ctx>(&vote.message)
            );

            return Ok(false);
        }

        return Ok(true);
    };

    if state.vote_extension_policy.is_disabled() {
        warn!(
            consensus.height = %state.height(),
            vote.height = %vote.height(),
            vote.round = %vote.round(),
            validator = %validator.address(),
            "Received non-nil precommit with disabled vote extension: {}",
            PrettyVote::<Ctx>(&vote.message)
        );

        return Ok(false);
    }

    let result = perform!(
        co,
        Effect::VerifyVoteExtension(
            vote.height(),
            vote.round(),
            value_id.clone(),
            validator.address().clone(),
            extension.clone(),
            validator.public_key().clone(),
            Default::default()
        ),
        Resume::VoteExtensionValidity(result) => result
    );

    if let Err(e) = result {
        warn!(
            consensus.height = %state.height(),
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

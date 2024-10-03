use crate::prelude::*;

use crate::types::ConsensusMsg;
use crate::util::pretty::PrettyVote;

use super::validator_set::get_validator_set;

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
    let validator_address = signed_vote.validator_address();

    let Some(validator_set) = get_validator_set(co, state, signed_vote.height()).await? else {
        debug!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
            validator = %validator_address,
            "Received vote for height without known validator set, dropping"
        );

        return Ok(false);
    };

    let Some(validator) = validator_set.get_by_address(validator_address) else {
        warn!(
            consensus.height = %consensus_height,
            vote.height = %vote_height,
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
            validator = %validator_address,
            "Received vote with invalid signature: {}", PrettyVote::<Ctx>(&signed_vote.message)
        );

        return Ok(false);
    }

    Ok(true)
}

pub async fn verify_signature<Ctx>(
    co: &Co<Ctx>,
    signed_msg: SignedMessage<Ctx, ConsensusMsg<Ctx>>,
    validator: &Ctx::Validator,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    let effect = Effect::VerifySignature(signed_msg, validator.public_key().clone());
    let valid = perform!(co, effect, Resume::SignatureValidity(valid) => valid);
    Ok(valid)
}

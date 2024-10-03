use crate::prelude::*;

use crate::types::ConsensusMsg;

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

use crate::prelude::*;

pub async fn get_validator_set<Ctx>(
    co: &Co<Ctx>,
    height: Ctx::Height,
) -> Result<Option<Ctx::ValidatorSet>, Error<Ctx>>
where
    Ctx: Context,
{
    perform!(co, Effect::GetValidatorSet(height),
        Resume::ValidatorSet(vs_height, validator_set) => {
            if vs_height == height {
                Ok(validator_set)
            } else {
                Err(Error::UnexpectedResume(
                    Resume::ValidatorSet(vs_height, validator_set),
                    "ValidatorSet for the current height"
                ))
            }
        }
    )
}

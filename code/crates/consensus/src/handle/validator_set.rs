use std::borrow::Cow;

use crate::prelude::*;

pub async fn get_validator_set<'a, Ctx>(
    co: &Co<Ctx>,
    state: &'a State<Ctx>,
    height: Ctx::Height,
) -> Result<Option<Cow<'a, Ctx::ValidatorSet>>, Error<Ctx>>
where
    Ctx: Context,
{
    if state.driver.height() == height {
        return Ok(Some(Cow::Borrowed(&state.driver.validator_set)));
    }

    perform!(co, Effect::GetValidatorSet(height),
        Resume::ValidatorSet(vs_height, validator_set) => {
            if vs_height == height {
                Ok(validator_set.map(Cow::Owned))
            } else {
                Err(Error::UnexpectedResume(
                    Resume::ValidatorSet(vs_height, validator_set),
                    "ValidatorSet for the given height"
                ))
            }
        }
    )
}

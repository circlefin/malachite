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
        return Ok(Some(Cow::Borrowed(state.driver.validator_set())));
    }

    perform!(co, Effect::GetValidatorSet(height, Default::default()),
        Resume::ValidatorSet(validator_set) => Ok(validator_set.map(Cow::Owned))
    )
}

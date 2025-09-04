use crate::prelude::*;

pub async fn get_validator_set<'a, Ctx>(
    _co: &Co<Ctx>,
    state: &'a State<Ctx>,
    height: Ctx::Height,
) -> Result<Option<&'a Ctx::ValidatorSet>, Error<Ctx>>
where
    Ctx: Context,
{
    if state.height() == height {
        Ok(Some(state.driver.validator_set()))
    } else {
        tracing::warn!(
            "Validator set for height {} is not available. Current height is {}.",
            height,
            state.driver.height()
        );

        Ok(None)
    }
}

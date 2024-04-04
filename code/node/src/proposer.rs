use malachite_common::{Context, Height, Round, Validator, ValidatorSet};

pub fn select_proposer<Ctx>(
    height: Ctx::Height,
    round: Round,
    validator_set: &Ctx::ValidatorSet,
) -> Option<&Ctx::Address>
where
    Ctx: Context,
{
    assert!(validator_set.count() > 0);
    assert!(round != Round::Nil && round.as_i64() >= 0);

    let height = height.as_u64() as usize;
    let round = round.as_i64() as usize;

    let proposer_index = (height - 1 + round) % validator_set.count();
    let proposer = validator_set.get_by_index(proposer_index);

    proposer.map(|v| v.address())
}

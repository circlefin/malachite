use malachite_common::{Context, Round};

pub trait ProposerSelector<Ctx>
where
    Ctx: Context,
{
    fn select_proposer(&mut self, round: Round, validator_set: &Ctx::ValidatorSet) -> Ctx::Address;
}

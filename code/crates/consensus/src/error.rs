use malachite_common::{Context, Round};

#[derive(Clone, Debug)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    ProposerNotFound(Ctx::Height, Round),
}

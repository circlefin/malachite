use genawaiter::sync as gen;
use genawaiter::GeneratorState;

use crate::{Effect, Error, Resume};

pub type Gen<Ctx, F> = gen::Gen<Effect<Ctx>, Resume<Ctx>, F>;
pub type Co<Ctx> = gen::Co<Effect<Ctx>, Resume<Ctx>>;
pub type CoState<Ctx> = GeneratorState<Effect<Ctx>, Result<(), Error<Ctx>>>;

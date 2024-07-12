use genawaiter::sync as gen;
use genawaiter::GeneratorState;

use crate::{Effect, Error, Resume};

pub use gen::Gen;

pub type Co<Ctx> = gen::Co<Effect<Ctx>, Resume<Ctx>>;
pub type CoResult<Ctx> = GeneratorState<Effect<Ctx>, Result<(), Error<Ctx>>>;

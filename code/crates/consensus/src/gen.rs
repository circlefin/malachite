use genawaiter::sync as gen;
use genawaiter::GeneratorState;

use crate::effect::{Effect, Resume};
use crate::error::Error;

pub use gen::Gen;

#[allow(private_interfaces)]
pub type Co<Ctx> = gen::Co<Effect<Ctx>, Resume<Ctx>>;

pub type CoResult<Ctx> = GeneratorState<Effect<Ctx>, Result<(), Error<Ctx>>>;

use core::fmt;

use malachite_common::Context;

/// The type of errors that can be yielded by the `Driver`.
#[derive(Clone, Debug)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// Proposer not found
    ProposerNotFound(Ctx::Address),

    /// Validator not found in validator set
    ValidatorNotFound(Ctx::Address),
}

impl<Ctx> fmt::Display for Error<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ProposerNotFound(addr) => write!(f, "Proposer not found: {addr}"),
            Error::ValidatorNotFound(addr) => write!(f, "Validator not found: {addr}"),
        }
    }
}

impl<Ctx> PartialEq for Error<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::ProposerNotFound(addr1), Error::ProposerNotFound(addr2)) => addr1 == addr2,
            (Error::ValidatorNotFound(addr1), Error::ValidatorNotFound(addr2)) => addr1 == addr2,
            _ => false,
        }
    }
}

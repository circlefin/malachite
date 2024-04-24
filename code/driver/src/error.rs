use core::fmt;

use derive_where::derive_where;

use malachite_common::{Context, Round};

/// The type of errors that can be yielded by the `Driver`.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// No proposer was set for this round
    NoProposer(Ctx::Height, Round),

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
            Error::NoProposer(height, round) => {
                write!(f, "No proposer set for height {height} at round {round}")
            }
            Error::ProposerNotFound(addr) => write!(f, "Proposer not found: {addr}"),
            Error::ValidatorNotFound(addr) => write!(f, "Validator not found: {addr}"),
        }
    }
}

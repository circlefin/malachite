use core::fmt;

use malachite_common::{Context, NilOrVal, Proposal, Value};

pub struct PrettyVal<'a, T>(pub NilOrVal<&'a T>);

impl<T> fmt::Display for PrettyVal<'_, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            NilOrVal::Nil => "Nil".fmt(f),
            NilOrVal::Val(v) => v.fmt(f),
        }
    }
}

pub struct PrettyVote<'a, Ctx: Context>(pub &'a Ctx::Vote);

impl<Ctx> fmt::Display for PrettyVote<'_, Ctx>
where
    Ctx: Context,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use malachite_common::Vote;

        write!(
            f,
            "{:?}(height: {}, round: {}, value: {}, from: {}",
            self.0.vote_type(),
            self.0.height(),
            self.0.round(),
            PrettyVal(self.0.value().as_ref()),
            self.0.validator_address(),
        )?;

        if let Some(e) = self.0.extension() {
            write!(f, ", extension: {:?} bytes", e.size_bytes())?;
        }

        write!(f, ")")
    }
}

pub struct PrettyProposal<'a, Ctx: Context>(pub &'a Ctx::Proposal);

impl<Ctx> fmt::Display for PrettyProposal<'_, Ctx>
where
    Ctx: Context,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Proposal(height: {}, round: {}, pol_round: {}, value: {}, from: {})",
            self.0.height(),
            self.0.round(),
            self.0.pol_round(),
            self.0.value().id(),
            self.0.validator_address()
        )
    }
}

use core::fmt;

use crate::{Context, Signature, Vote};

// TODO: Do we need to abstract over `SignedVote` as well?

pub struct SignedVote<Ctx>
where
    Ctx: Context,
{
    pub vote: Ctx::Vote,
    pub signature: Signature<Ctx>,
}

impl<Ctx> SignedVote<Ctx>
where
    Ctx: Context,
{
    pub fn new(vote: Ctx::Vote, signature: Signature<Ctx>) -> Self {
        Self { vote, signature }
    }

    pub fn validator_address(&self) -> &Ctx::Address {
        self.vote.validator_address()
    }
}

// NOTE: We have to derive these instances manually, otherwise
//       the compiler would infer a Clone/Debug/PartialEq/Eq bound on `Ctx`,
//       which may not hold for all contexts.

impl<Ctx: Context> Clone for SignedVote<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clone(&self) -> Self {
        Self {
            vote: self.vote.clone(),
            signature: self.signature.clone(),
        }
    }
}

impl<Ctx: Context> fmt::Debug for SignedVote<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedVote")
            .field("vote", &self.vote)
            .field("signature", &self.signature)
            .finish()
    }
}

impl<Ctx: Context> PartialEq for SignedVote<Ctx> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn eq(&self, other: &Self) -> bool {
        self.vote == other.vote && self.signature == other.signature
    }
}

impl<Ctx: Context> Eq for SignedVote<Ctx> {}

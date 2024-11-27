use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{Context, SignedVote};

/// Represents a signature for a certificate, including the address and the signature itself.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct VoteSet<Ctx: Context> {
    /// The set of votes at height and round
    pub vote_set: Vec<SignedVote<Ctx>>,
}

impl<Ctx: Context> VoteSet<Ctx> {
    /// Create a new `VoteSet`
    pub fn new(vote_set: Vec<SignedVote<Ctx>>) -> Self {
        Self { vote_set }
    }
}

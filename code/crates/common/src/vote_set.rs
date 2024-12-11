use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{Context, SignedVote};

/// Represents a signature for a certificate, including the address and the signature itself.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct VoteSet<Ctx: Context> {
    /// The set of votes at height and round
    pub votes: Vec<SignedVote<Ctx>>,
}

impl<Ctx: Context> VoteSet<Ctx> {
    /// Create a new `VoteSet`
    pub fn new(votes: Vec<SignedVote<Ctx>>) -> Self {
        Self { votes }
    }

    /// Return the number of votes in the `VoteSet`
    pub fn len(&self) -> usize {
        self.votes.len()
    }

    /// Return whether or not the `VoteSet` is empty
    pub fn is_empty(&self) -> bool {
        self.votes.is_empty()
    }
}

use alloc::vec::Vec;
use derive_where::derive_where;

use crate::{Context, SignedVote};

/// A certificate is a collection of commits
/// TODO - will optimize later
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Certificate<Ctx: Context> {
    /// The commits
    pub commits: Vec<SignedVote<Ctx>>,
}

impl<Ctx: Context> Certificate<Ctx> {
    /// Creates a certificate
    pub fn new(commits: Vec<SignedVote<Ctx>>) -> Self {
        Self { commits }
    }
}

use core::fmt::Debug;

use alloc::vec::Vec;
use bytes::Bytes;
use derive_where::derive_where;

use crate::{Context, SignedExtension};

/// A set of vote extensions.
#[derive_where(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteExtensions<Ctx: Context> {
    /// The vote extensions together with the address of their proposer.
    pub extensions: Vec<(Ctx::Address, SignedExtension<Ctx>)>,
}

impl<Ctx: Context> VoteExtensions<Ctx> {
    /// Creates a new set of vote extensions.
    pub fn new(mut extensions: Vec<(Ctx::Address, SignedExtension<Ctx>)>) -> Self {
        // Sort vote extensions by their proposer's address
        extensions.sort_by(|(a, _), (b, _)| a.cmp(b));

        Self { extensions }
    }

    /// Returns the size of the extensions in bytes.
    pub fn size_bytes(&self) -> usize {
        self.extensions.iter().map(|(_, e)| e.size_bytes()).sum()
    }
}

/// Vote extensions allows applications to extend the pre-commit vote with arbitrary data.
/// This allows applications to force their validators to do more than just validate blocks within consensus.
pub trait Extension
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
{
    /// Returns the size of the extension in bytes.
    fn size_bytes(&self) -> usize;
}

impl Extension for () {
    fn size_bytes(&self) -> usize {
        0
    }
}

impl Extension for Vec<u8> {
    fn size_bytes(&self) -> usize {
        self.len()
    }
}

impl Extension for Bytes {
    fn size_bytes(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> Extension for [u8; N] {
    fn size_bytes(&self) -> usize {
        N
    }
}

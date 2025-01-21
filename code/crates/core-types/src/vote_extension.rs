use core::fmt::Debug;

use alloc::vec::Vec;
use bytes::Bytes;

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

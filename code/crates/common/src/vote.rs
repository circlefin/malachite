use core::fmt::Debug;

use bytes::Bytes;

use crate::{Context, NilOrVal, Round, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VoteType {
    /// Votes for values which validators observe are valid for a given round.
    Prevote,

    /// Votes to commit to a particular value for a given round.
    Precommit,
}

/// Vote extensions allows applications to extend the pre-commit vote with arbitrary data.
/// This allows applications to force their validators to do more than just validate blocks within consensus.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Extension {
    /// This data is opaque to the consensus algorithm but can contain application-specific information.
    pub data: Bytes,
}

impl Extension {
    /// Create a new extension with the given data.
    pub fn new(data: Bytes) -> Self {
        Self { data }
    }

    /// Get the size of the extension in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

impl<A> From<A> for Extension
where
    A: Into<Bytes>,
{
    fn from(data: A) -> Self {
        Self { data: data.into() }
    }
}

/// Defines the requirements for a vote.
///
/// Votes are signed messages from validators for a particular value which
/// include information about the validator signing it.
pub trait Vote<Ctx>
where
    Self: Clone + Debug + Eq + Ord + Send + Sync + 'static,
    Ctx: Context,
{
    /// The height for which the vote is for.
    fn height(&self) -> Ctx::Height;

    /// The round for which the vote is for.
    fn round(&self) -> Round;

    /// Get a reference to the value being voted for.
    fn value(&self) -> &NilOrVal<<Ctx::Value as Value>::Id>;

    /// Take ownership of the value being voted for.
    fn take_value(self) -> NilOrVal<<Ctx::Value as Value>::Id>;

    /// The type of vote.
    fn vote_type(&self) -> VoteType;

    /// Address of the validator who issued this vote
    fn validator_address(&self) -> &Ctx::Address;

    /// Votes extensions
    fn extension(&self) -> Option<&Extension>;

    /// Extend this vote with an extension, overriding any existing extension.
    fn extend(self, extension: Extension) -> Self;
}

use core::fmt::Debug;

use crate::{Context, NilOrVal, Round, SignedExtension, Value};

/// A type of vote.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "borsh",
    derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize)
)]
pub enum VoteType {
    /// Votes for values which validators observe are valid for a given round.
    Prevote,

    /// Votes to commit to a particular value for a given round.
    Precommit,
}

/// Defines the requirements for a vote.
///
/// Votes are signed messages from validators for a particular value which
/// include information about the validator signing it.
pub trait Vote<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync + 'static,
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
    fn extension(&self) -> Option<&SignedExtension<Ctx>>;

    /// Return an owned reference to this vote's extensions,
    /// removing them from the vote in the process.
    fn take_extension(&mut self) -> Option<SignedExtension<Ctx>>;

    /// Extend this vote with an extension, overriding any existing extension.
    fn extend(self, extension: SignedExtension<Ctx>) -> Self;
}

use core::fmt::{Debug, Display};

use crate::{Context, PublicKey};

/// Voting power held by a validator.
///
/// TODO: Introduce newtype
pub type VotingPower = u64;

/// Defines the requirements for an address.
pub trait Address
where
    Self: Clone + Debug + Display + Eq + Ord + Send + Sync,
{
}

/// Defines the requirements for a validator.
pub trait Validator<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync,
    Ctx: Context,
{
    /// The address of the validator, typically derived from its public key.
    fn address(&self) -> &Ctx::Address;

    /// The public key of the validator, used to verify signatures.
    fn public_key(&self) -> &PublicKey<Ctx>;

    /// The voting power held by the validaror.
    fn voting_power(&self) -> VotingPower;
}

/// Defines the requirements for a validator set.
///
/// A validator set is a collection of validators.
///
/// # Important
/// The validators must be unique and sorted in a deterministic order.
///
/// Such an ordering can be defined as in CometBFT:
/// - first by validator power (descending)
/// - then lexicographically by address (ascending)
pub trait ValidatorSet<Ctx>
where
    Self: Clone + Debug + Eq + Send + Sync,
    Ctx: Context,
{
    /// The number of validators in the set.
    fn count(&self) -> usize;

    /// The total voting power of the validator set.
    fn total_voting_power(&self) -> VotingPower;

    /// Get the validator with the given address.
    fn get_by_address(&self, address: &Ctx::Address) -> Option<&Ctx::Validator>;

    /// Get the validator at the given index.
    fn get_by_index(&self, index: usize) -> Option<&Ctx::Validator>;
}

use core::fmt::{Debug, Display};

use crate::{Context, PublicKey};

/// Voting power held by a validator.
///
/// TODO: Introduce newtype
pub type VotingPower = u64;

/// Defines the requirements for an address.
pub trait Address
where
    Self: Clone + Debug + Display + Eq + Ord,
{
}

/// Defines the requirements for a validator.
pub trait Validator<Ctx>
where
    Self: Clone + Debug + PartialEq + Eq,
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
pub trait ValidatorSet<Ctx>
where
    Self: Clone + Debug,
    Ctx: Context,
{
    /// The total voting power of the validator set.
    fn total_voting_power(&self) -> VotingPower;

    /// Get the validator with the given address.
    fn get_by_address(&self, address: &Ctx::Address) -> Option<&Ctx::Validator>;
}

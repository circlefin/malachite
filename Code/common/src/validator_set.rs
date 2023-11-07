use core::fmt::Debug;

use crate::{Context, PublicKey};

/// Voting power held by a validator.
///
/// TODO: Do we need to abstract over this as well?
pub type VotingPower = u64;

/// Defines the requirements for an address.
///
/// TODO: Keep this trait or just add the bounds to Consensus::Address?
pub trait Address
where
    Self: Clone + Debug + PartialEq + Eq,
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
    Ctx: Context,
{
    /// The total voting power of the validator set.
    fn total_voting_power(&self) -> VotingPower;

    /// The proposer in the validator set.
    fn get_proposer(&self) -> &Ctx::Validator;

    /// Get the validator with the given public key.
    fn get_by_public_key(&self, public_key: &PublicKey<Ctx>) -> Option<&Ctx::Validator>;

    /// Get the validator with the given address.
    fn get_by_address(&self, address: &Ctx::Address) -> Option<&Ctx::Validator>;
}

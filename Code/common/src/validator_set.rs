use core::fmt::Debug;

use crate::Consensus;

/// Voting power held by a validator.
///
/// TODO: Do we need to abstract over this as well?
pub type VotingPower = u64;

/// Defines the requirements for a public key type.
pub trait PublicKey
where
    Self: Clone + Debug + PartialEq + Eq,
{
}

/// Defines the requirements for an address.
///
/// TODO: Keep this trait or just add the bounds to Consensus::Address?
pub trait Address
where
    Self: Clone + Debug + PartialEq + Eq,
{
}

/// Defines the requirements for a validator.
pub trait Validator<C>
where
    Self: Clone + Debug + PartialEq + Eq,
    C: Consensus,
{
    /// The address of the validator, typically derived from its public key.
    fn address(&self) -> &C::Address;

    /// The public key of the validator, used to verify signatures.
    fn public_key(&self) -> &C::PublicKey;

    /// The voting power held by the validaror.
    fn voting_power(&self) -> VotingPower;
}

/// Defines the requirements for a validator set.
///
/// A validator set is a collection of validators.
pub trait ValidatorSet<C>
where
    C: Consensus,
{
    /// The total voting power of the validator set.
    fn total_voting_power(&self) -> VotingPower;

    /// The proposer in the validator set.
    fn get_proposer(&self) -> C::Validator;

    /// Get the validator with the given public key.
    fn get_by_public_key(&self, public_key: &C::PublicKey) -> Option<&C::Validator>;

    /// Get the validator with the given address.
    fn get_by_address(&self, address: &C::Address) -> Option<&C::Validator>;
}

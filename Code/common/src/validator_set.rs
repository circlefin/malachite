use core::fmt::Debug;

use crate::Consensus;

// TODO: Do we need to abstract over this as well?
pub type VotingPower = u64;

pub trait PublicKey
where
    Self: Clone + Debug + PartialEq + Eq,
{
    fn hash(&self) -> u64; // FIXME: Make the hash type generic
}

// TODO: Keep this trait or just add the bounds to Consensus::Address?
pub trait Address
where
    Self: Clone + Debug + PartialEq + Eq,
{
}

pub trait Validator<C>
where
    Self: Clone + Debug + PartialEq + Eq,
    C: Consensus,
{
    fn address(&self) -> &C::Address;
    fn public_key(&self) -> &C::PublicKey;
    fn voting_power(&self) -> VotingPower;
}

pub trait ValidatorSet<C>
where
    C: Consensus,
{
    fn total_voting_power(&self) -> VotingPower;
    fn get_proposer(&self) -> C::Validator;
    fn get_by_public_key(&self, public_key: &C::PublicKey) -> Option<&C::Validator>;
    fn get_by_address(&self, address: &C::Address) -> Option<&C::Validator>;
}

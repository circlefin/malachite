use crate::{
    Address, Height, Proposal, PublicKey, Round, Validator, ValidatorSet, Value, ValueId, Vote,
};

/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Consensus
where
    Self: Sized,
{
    type Address: Address;
    type Height: Height;
    type Proposal: Proposal<Self>;
    type PublicKey: PublicKey;
    type Validator: Validator<Self>;
    type ValidatorSet: ValidatorSet<Self>;
    type Value: Value;
    type Vote: Vote<Self>;

    // FIXME: Remove this and thread it through where necessary
    const DUMMY_ADDRESS: Self::Address;

    // FIXME: Remove altogether
    const DUMMY_VALUE: Self::Value;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;
}

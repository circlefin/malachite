use crate::{
    Address, Height, Proposal, PublicKey, Round, Validator, ValidatorSet, Value, ValueId, Vote,
};

pub trait Consensus
where
    Self: Sized,
{
    type Address: Address;
    type Height: Height;
    type Proposal: Proposal<Self>;
    type PublicKey: PublicKey;
    type ValidatorSet: ValidatorSet<Self>;
    type Validator: Validator<Self>;
    type Value: Value;
    type Vote: Vote<Self>;

    // FIXME: Remove this and thread it through where necessary
    const DUMMY_ADDRESS: Self::Address;

    // FIXME: Remove
    const DUMMY_VALUE: Self::Value;

    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
    ) -> Self::Proposal;

    fn new_prevote(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    fn new_precommit(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;
}

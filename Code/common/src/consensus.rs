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

pub mod test {
    use crate::height::test::*;
    use crate::proposal::test::*;
    use crate::validator_set::test::*;
    use crate::value::test::*;
    use crate::vote::test::*;
    use crate::Round;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct TestConsensus;

    impl super::Consensus for TestConsensus {
        type Address = Address;
        type Height = Height;
        type Proposal = Proposal;
        type PublicKey = PublicKey;
        type ValidatorSet = ValidatorSet;
        type Validator = Validator;
        type Value = Value;
        type Vote = Vote;

        const DUMMY_ADDRESS: Address = Address::new(42);

        const DUMMY_VALUE: Self::Value = Value::new(9999);

        fn new_proposal(height: Height, round: Round, value: Value, pol_round: Round) -> Proposal {
            Proposal::new(height, round, value, pol_round)
        }

        fn new_prevote(round: Round, value_id: Option<ValueId>, address: Address) -> Vote {
            Vote::new_prevote(round, value_id, address)
        }

        fn new_precommit(round: Round, value_id: Option<ValueId>, address: Address) -> Vote {
            Vote::new_precommit(round, value_id, address)
        }
    }
}

use malachite_common::Consensus;
use malachite_common::Round;

use crate::height::*;
use crate::proposal::*;
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestConsensus;

impl Consensus for TestConsensus {
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

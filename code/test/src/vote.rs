use malachite_proto::Protobuf;
use signature::Signer;

use malachite_common::{NilOrVal, Round, SignedVote, VoteType};

use crate::{Address, Height, PrivateKey, TestContext, ValueId};

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub typ: VoteType,
    pub height: Height,
    pub round: Round,
    pub value: NilOrVal<ValueId>,
    pub validator_address: Address,
}

impl Vote {
    pub fn new_prevote(
        height: Height,
        round: Round,
        value: NilOrVal<ValueId>,
        validator_address: Address,
    ) -> Self {
        Self {
            typ: VoteType::Prevote,
            height,
            round,
            value,
            validator_address,
        }
    }

    pub fn new_precommit(
        height: Height,
        round: Round,
        value: NilOrVal<ValueId>,
        address: Address,
    ) -> Self {
        Self {
            typ: VoteType::Precommit,
            height,
            round,
            value,
            validator_address: address,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn signed(self, private_key: &PrivateKey) -> SignedVote<TestContext> {
        let signature = private_key.sign(&self.to_bytes());

        SignedVote {
            vote: self,
            signature,
        }
    }
}

impl malachite_common::Vote<TestContext> for Vote {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &NilOrVal<ValueId> {
        &self.value
    }

    fn take_value(self) -> NilOrVal<ValueId> {
        self.value
    }

    fn vote_type(&self) -> VoteType {
        self.typ
    }

    fn validator_address(&self) -> &Address {
        &self.validator_address
    }
}

impl TryFrom<malachite_proto::Vote> for Vote {
    type Error = String;

    fn try_from(vote: malachite_proto::Vote) -> Result<Self, Self::Error> {
        Ok(Self {
            typ: malachite_proto::VoteType::try_from(vote.vote_type)
                .unwrap()
                .try_into()
                .unwrap(), // infallible
            height: vote.height.unwrap().try_into().unwrap(), // infallible
            round: vote.round.unwrap().try_into().unwrap(),   // infallible
            value: match vote.value {
                Some(value) => NilOrVal::Val(value.try_into().unwrap()), // FIXME
                None => NilOrVal::Nil,
            },
            validator_address: vote.validator_address.unwrap().try_into().unwrap(), // FIXME
        })
    }
}

impl From<Vote> for malachite_proto::Vote {
    fn from(vote: Vote) -> malachite_proto::Vote {
        malachite_proto::Vote {
            vote_type: i32::from(malachite_proto::VoteType::from(vote.typ)),
            height: Some(vote.height.into()),
            round: Some(vote.round.into()),
            value: match vote.value {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.into()),
            },
            validator_address: Some(vote.validator_address.into()),
        }
    }
}

impl malachite_proto::Protobuf for Vote {
    type Proto = malachite_proto::Vote;
}

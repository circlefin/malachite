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

    // TODO: Use a canonical vote
    pub fn to_bytes(&self) -> Vec<u8> {
        let vtpe = match self.typ {
            VoteType::Prevote => 0,
            VoteType::Precommit => 1,
        };

        let mut bytes = vec![vtpe];
        bytes.extend_from_slice(&self.round.as_i64().to_be_bytes());
        bytes.extend_from_slice(
            self.value
                .as_ref()
                .map(|v| v.as_u64().to_be_bytes())
                .value_or_default()
                .as_slice(),
        );
        bytes
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

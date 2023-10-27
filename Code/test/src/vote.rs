use malachite_common::{Round, SignedVote, VoteType};
use signature::Signer;

use crate::{Address, PrivateKey, TestConsensus, ValueId};

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub typ: VoteType,
    pub round: Round,
    pub value: Option<ValueId>,
}

impl Vote {
    pub fn new_prevote(round: Round, value: Option<ValueId>) -> Self {
        Self {
            typ: VoteType::Prevote,
            round,
            value,
        }
    }

    pub fn new_precommit(round: Round, value: Option<ValueId>) -> Self {
        Self {
            typ: VoteType::Precommit,
            round,
            value,
        }
    }

    // TODO: Use the canonical vote
    pub fn to_bytes(&self) -> Vec<u8> {
        let vtpe = match self.typ {
            VoteType::Prevote => 0,
            VoteType::Precommit => 1,
        };

        let mut bytes = vec![vtpe];
        bytes.extend_from_slice(&self.round.as_i64().to_be_bytes());
        bytes.extend_from_slice(
            &self
                .value
                .map(|v| v.as_u64().to_be_bytes())
                .unwrap_or_default(),
        );
        bytes
    }

    pub fn signed(self, private_key: &PrivateKey) -> SignedVote<TestConsensus> {
        let address = Address::from_public_key(&private_key.public_key());
        let signature = private_key.sign(&self.to_bytes());

        SignedVote {
            vote: self,
            address,
            signature,
        }
    }
}

impl malachite_common::Vote<TestConsensus> for Vote {
    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> Option<&ValueId> {
        self.value.as_ref()
    }

    fn take_value(self) -> Option<ValueId> {
        self.value
    }

    fn vote_type(&self) -> VoteType {
        self.typ
    }
}

use malachite_common::{Round, VoteType};
use malachite_consensus::signed_vote::SignedVote;

use crate::{Address, TestConsensus, ValueId};

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

    pub fn signed(self, address: Address) -> SignedVote<TestConsensus> {
        SignedVote {
            vote: self,
            address,
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

    fn vote_type(&self) -> VoteType {
        self.typ
    }
}

use malachite_common::{Round, VoteType};

use crate::{Address, TestConsensus, ValueId};

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub typ: VoteType,
    pub round: Round,
    pub value: Option<ValueId>,
    pub address: Address,
}

impl Vote {
    pub fn new_prevote(round: Round, value: Option<ValueId>, address: Address) -> Self {
        Self {
            typ: VoteType::Prevote,
            round,
            value,
            address,
        }
    }

    pub fn new_precommit(round: Round, value: Option<ValueId>, address: Address) -> Self {
        Self {
            typ: VoteType::Precommit,
            round,
            value,
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

    fn address(&self) -> &Address {
        &self.address
    }

    fn set_address(&mut self, address: Address) {
        self.address = address;
    }
}

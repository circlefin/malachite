use crate::{Address, Round, ValueId};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

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

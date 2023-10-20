use crate::{Address, Round, Value};

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
    pub value: Option<Value>,
    pub address: Address,
}

impl Vote {
    pub fn new_prevote(round: Round, value: Option<Value>, address: Address) -> Self {
        Self {
            typ: VoteType::Prevote,
            round,
            value,
            address,
        }
    }

    pub fn new_precommit(round: Round, value: Option<Value>, address: Address) -> Self {
        Self {
            typ: VoteType::Precommit,
            round,
            value,
            address,
        }
    }
}

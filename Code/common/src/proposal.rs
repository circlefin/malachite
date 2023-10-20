use crate::{Round, Value};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub round: Round,
    pub value: Value,
    pub pol_round: Round,
}

impl Proposal {
    pub fn new(round: Round, value: Value, pol_round: Round) -> Self {
        Self {
            round,
            value,
            pol_round,
        }
    }
}

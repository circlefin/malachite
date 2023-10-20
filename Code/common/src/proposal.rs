use crate::{Round, Value};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub round: Round,
    pub value: Value,
    pub pol_round: Round,
}

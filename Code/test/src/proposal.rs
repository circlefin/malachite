use malachite_common::Round;

use crate::{Height, TestContext, Value};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value: Value,
    pub pol_round: Round,
}

impl Proposal {
    pub fn new(height: Height, round: Round, value: Value, pol_round: Round) -> Self {
        Self {
            height,
            round,
            value,
            pol_round,
        }
    }
}

impl malachite_common::Proposal<TestContext> for Proposal {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &Value {
        &self.value
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }
}

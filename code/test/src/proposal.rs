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

impl TryFrom<malachite_proto::Proposal> for Proposal {
    type Error = String;

    fn try_from(proposal: malachite_proto::Proposal) -> Result<Self, Self::Error> {
        Ok(Self {
            height: proposal.height.unwrap().try_into().unwrap(), // infallible
            round: proposal.round.unwrap().try_into().unwrap(),   // infallible
            value: proposal.value.unwrap().try_into().unwrap(),   // FIXME
            pol_round: proposal.pol_round.unwrap().try_into().unwrap(), // infallible
        })
    }
}

impl From<Proposal> for malachite_proto::Proposal {
    fn from(proposal: Proposal) -> malachite_proto::Proposal {
        malachite_proto::Proposal {
            height: Some(proposal.height.into()),
            round: Some(proposal.round.into()),
            value: Some(proposal.value.into()),
            pol_round: Some(proposal.pol_round.into()),
        }
    }
}

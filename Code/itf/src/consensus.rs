use itf::{ItfBigInt, ItfMap};
use serde::Deserialize;

use crate::deserializers as de;

pub type Address = String;
pub type Value = String;
pub type Step = String;
pub type Round = ItfBigInt;
pub type Height = ItfBigInt;

#[derive(Clone, Debug, Deserialize)]
pub enum Timeout {
    #[serde(rename = "timeoutPrevote")]
    Prevote,

    #[serde(rename = "timeoutPrecommit")]
    Precommit,

    #[serde(rename = "timeoutPropose")]
    Propose,
}

#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub system: System,

    #[serde(rename = "_Event")]
    pub event: Event,

    #[serde(rename = "_Result")]
    pub result: Result,
}

#[derive(Clone, Debug, Deserialize)]
pub struct System(ItfMap<Address, ConsensusState>);

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "name")]
pub enum Event {
    Initial,
    NewRound {
        height: Height,
        round: Round,
    },
    Proposal {
        height: Height,
        round: Round,
        value: Value,
    },
    ProposalAndPolkaAndValid {
        height: Height,
        round: Round,
        value: Value,
    },
    ProposalAndCommitAndValid {
        height: Height,
        round: Round,
        value: Value,
    },
    NewHeight {
        height: Height,
        round: Round,
    },
    NewRoundProposer {
        height: Height,
        round: Round,
        value: Value,
    },
    PolkaNil {
        height: Height,
        round: Round,
        value: Value,
    },
    PolkaAny {
        height: Height,
        round: Round,
        value: Value,
    },
    PrecommitAny {
        height: Height,
        round: Round,
        value: Value,
    },
    TimeoutPrevote {
        height: Height,
        round: Round,
    },
    TimeoutPrecommit {
        height: Height,
        round: Round,
        value: Value,
    },
    TimeoutPropose {
        height: Height,
        round: Round,
        value: Value,
    },
    ProposalInvalid {
        height: Height,
        round: Round,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Result {
    pub name: String,
    #[serde(deserialize_with = "de::proposal_or_none")]
    pub proposal: Option<Proposal>,
    #[serde(deserialize_with = "de::vote_message_or_none")]
    pub vote_message: Option<VoteMessage>,
    #[serde(deserialize_with = "de::empty_string_as_none")]
    pub timeout: Option<Timeout>,
    #[serde(deserialize_with = "de::empty_string_as_none")]
    pub decided: Option<Value>,
    #[serde(deserialize_with = "de::minus_one_as_none")]
    pub skip_round: Option<Round>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub src: Address,
    pub height: Height,
    pub round: Round,
    pub proposal: Value,
    pub valid_round: Round,
}

impl Proposal {
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
            && self.proposal.is_empty()
            && self.height == ItfBigInt::from(-1)
            && self.round == ItfBigInt::from(-1)
            && self.valid_round == ItfBigInt::from(-1)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteMessage {
    pub src: Address,
    pub height: Height,
    pub round: Round,
    pub step: Step,
    pub id: Value,
}

impl VoteMessage {
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
            && self.id.is_empty()
            && self.height == ItfBigInt::from(-1)
            && self.round == ItfBigInt::from(-1)
            && self.step.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusState {
    pub p: Address,
    pub height: Height,
    pub round: Round,
    pub step: Step,

    #[serde(deserialize_with = "de::minus_one_as_none")]
    pub locked_round: Option<Round>,
    #[serde(deserialize_with = "de::empty_string_as_none")]
    pub locked_value: Option<Value>,
    #[serde(deserialize_with = "de::minus_one_as_none")]
    pub valid_round: Option<Round>,
    #[serde(deserialize_with = "de::empty_string_as_none")]
    pub valid_value: Option<Value>,
}

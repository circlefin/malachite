use itf::de::{As, Integer};
use malachite_round::state::Step as RoundStep;
use serde::Deserialize;

pub type Height = i64;
pub type Weight = i64;
pub type Round = i64;
pub type Address = String;
pub type NonNilValue = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum Value {
    Nil,
    Val(NonNilValue),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub src_address: Address,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub proposal: NonNilValue,
    #[serde(with = "As::<Integer>")]
    pub valid_round: Round,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum VoteType {
    Prevote,
    Precommit,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vote {
    pub vote_type: VoteType,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub value_id: Value,
    pub src_address: Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum Step {
    #[serde(rename = "NewRoundStep")]
    NewRound,
    #[serde(rename = "ProposeStep")]
    Propose,
    #[serde(rename = "PrevoteStep")]
    Prevote,
    #[serde(rename = "PrecommitStep")]
    Precommit,
    #[serde(rename = "DecidedStep")]
    Decided,
}

impl Step {
    pub fn to_round_step(&self) -> RoundStep {
        match self {
            Step::NewRound => RoundStep::NewRound,
            Step::Propose => RoundStep::Propose,
            Step::Prevote => RoundStep::Prevote,
            Step::Precommit => RoundStep::Precommit,
            Step::Decided => RoundStep::Commit,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum Timeout {
    #[serde(rename = "ProposeTimeout")]
    Propose,

    #[serde(rename = "PrevoteTimeout")]
    Prevote,

    #[serde(rename = "PrecommitTimeout")]
    Precommit,
}

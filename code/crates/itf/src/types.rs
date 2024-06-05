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

impl Value {
    pub fn fold<A>(&self, nil: A, val: impl FnOnce(&NonNilValue) -> A) -> A {
        match self {
            Value::Nil => nil,
            Value::Val(value) => val(value),
        }
    }
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

impl VoteType {
    pub fn to_common(&self) -> malachite_common::VoteType {
        match self {
            VoteType::Prevote => malachite_common::VoteType::Prevote,
            VoteType::Precommit => malachite_common::VoteType::Precommit,
        }
    }
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
    #[serde(rename = "NoStep")]
    None,
    #[serde(rename = "UnstartedStep")]
    Unstarted,
    #[serde(rename = "ProposeStep")]
    Propose,
    #[serde(rename = "PrevoteStep")]
    Prevote,
    #[serde(rename = "PrecommitStep")]
    Precommit,
    #[serde(rename = "CommitStep")]
    Commit,
}

impl Step {
    pub fn to_round_step(&self) -> Option<RoundStep> {
        match self {
            Step::None => None,
            Step::Unstarted => Some(RoundStep::Unstarted),
            Step::Propose => Some(RoundStep::Propose),
            Step::Prevote => Some(RoundStep::Prevote),
            Step::Precommit => Some(RoundStep::Precommit),
            Step::Commit => Some(RoundStep::Commit),
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

impl Timeout {
    pub fn to_common(&self) -> malachite_common::TimeoutStep {
        match self {
            Timeout::Propose => malachite_common::TimeoutStep::Propose,
            Timeout::Prevote => malachite_common::TimeoutStep::Prevote,
            Timeout::Precommit => malachite_common::TimeoutStep::Precommit,
        }
    }
}

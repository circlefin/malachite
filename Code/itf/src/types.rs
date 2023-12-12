use itf::de::{As, Integer};
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

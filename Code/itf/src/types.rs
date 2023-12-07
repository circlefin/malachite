use serde::Deserialize;

pub type Height = i64;
pub type Weight = i64;
pub type Round = i64;
pub type Address = String;
pub type NonNilValue = String;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum VoteType {
    Prevote,
    Precommit,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum Value {
    Nil,
    Val(String),
}

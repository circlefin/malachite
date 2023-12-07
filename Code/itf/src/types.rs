use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
pub struct EmptyObject {}

pub type Height = i64;
pub type Weight = i64;
pub type Round = i64;
pub type Address = String;
pub type NonNilValue = String;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteTypeTag {
    pub tag: String, // "Precommit" or "Prevote"
    pub value: EmptyObject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

impl TryFrom<VoteTypeTag> for VoteType {
    type Error = String;
    fn try_from(v: VoteTypeTag) -> Result<Self, Self::Error> {
        match v.tag.as_str() {
            "Prevote" => Ok(VoteType::Prevote),
            "Precommit" => Ok(VoteType::Precommit),
            _ => todo!(), // error
        }
    }
}

pub type SerdeVoteType = serde_with::TryFromInto<VoteTypeTag>;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(untagged)]
pub enum ValueValues {
    Val(NonNilValue),
    Nil(EmptyObject),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ValueTag {
    pub tag: String, // "Nil" or "Val"
    pub value: ValueValues,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
pub enum Value {
    Nil,
    Val(String),
}

impl TryFrom<ValueTag> for Value {
    type Error = String;
    fn try_from(v: ValueTag) -> Result<Self, Self::Error> {
        match v.tag.as_str() {
            "Nil" => Ok(Value::Nil),
            "Val" => match v.value {
                ValueValues::Val(v) => Ok(Value::Val(v)),
                ValueValues::Nil(_) => todo!(), // error
            },
            _ => todo!(), // error
        }
    }
}

pub type SerdeValue = serde_with::TryFromInto<ValueTag>;

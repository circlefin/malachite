use itf::de::{As, Integer, Same};
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::types::{Weight, Round, EmptyObject, VoteType, Height, Value, Address, NonNilValue, SerdeValue, SerdeVoteType};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum WeightedVoteValues {
    #[serde(with = "As::<(Same, Integer, Integer)>")]
    WV((Vote, Weight, Round)),
    NoWeightedVote(EmptyObject)
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WeightedVoteTag {
    pub tag: String,
    pub value: WeightedVoteValues,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeightedVote {
    NoWeightedVote,
    WV(Vote, Weight, Round),
}

impl TryFrom<WeightedVoteTag> for WeightedVote {
    type Error = String;
    fn try_from(v: WeightedVoteTag) -> Result<Self, Self::Error> {
        match v.tag.as_str() {
            "NoWeightedVote" => Ok(WeightedVote::NoWeightedVote),
            "WV" => match v.value {
                WeightedVoteValues::WV((v, w, r)) => Ok(WeightedVote::WV(v,w,r)),
                WeightedVoteValues::NoWeightedVote(_) => todo!(),
            }
            _ => todo!(), // error
        }
    }
}

pub type SerdeWeightedVote = serde_with::TryFromInto<WeightedVoteTag>;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(untagged)]
pub enum VoteKeeperOutputValues {
    None(EmptyObject),
    #[serde(with = "As::<Integer>")]
    Round(Round),
    #[serde(with = "As::<(Integer, Same)>")]
    RoundValue((Round, NonNilValue)),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
pub struct VoteKeeperOutputTag {
    pub tag: String,
    pub value: VoteKeeperOutputValues,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VoteKeeperOutput {
    NoVKOutput, 
    PolkaAnyVKOutput(Round),
    PolkaNilVKOutput(Round),
    PolkaValueVKOutput(Round, NonNilValue),
    PrecommitAnyVKOutput(Round),
    PrecommitValueVKOutput(Round, NonNilValue),
    SkipVKOutput(Round),
}

impl TryFrom<VoteKeeperOutputTag> for VoteKeeperOutput {
    type Error = String;
    fn try_from(v: VoteKeeperOutputTag) -> Result<Self, Self::Error> {
        match v.tag.as_str() {
            "NoVKOutput" => Ok(VoteKeeperOutput::NoVKOutput),
            "PolkaAnyVKOutput" => match v.value {
                VoteKeeperOutputValues::Round(r) => Ok(VoteKeeperOutput::PolkaAnyVKOutput(r)),
                _ => todo!(), // error
            }
            "PolkaNilVKOutput" => match v.value {
                VoteKeeperOutputValues::Round(r) => Ok(VoteKeeperOutput::PolkaNilVKOutput(r)),
                _ => todo!(), // error
            }
            "PolkaValueVKOutput" => match v.value {
                VoteKeeperOutputValues::RoundValue((r,v)) => Ok(VoteKeeperOutput::PolkaValueVKOutput(r,v)),
                _ => todo!(), // error
            }
            "PrecommitAnyVKOutput" => match v.value {
                VoteKeeperOutputValues::Round(r) => Ok(VoteKeeperOutput::PrecommitAnyVKOutput(r)),
                _ => todo!(), // error
            }
            "PrecommitValueVKOutput" => match v.value {
                VoteKeeperOutputValues::RoundValue((r,v)) => Ok(VoteKeeperOutput::PrecommitValueVKOutput(r,v)),
                _ => todo!(), // error
            }
            "SkipVKOutput" => match v.value {
                VoteKeeperOutputValues::Round(r) => Ok(VoteKeeperOutput::SkipVKOutput(r)),
                _ => todo!(), // error
            }
            _ => todo!(), // error
        }
    }
}

pub type SerdeVoteKeeperOutput = serde_with::TryFromInto<VoteKeeperOutputTag>;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookkeeper {
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub total_weight: Weight,
    #[serde(with = "As::<HashMap<Integer, Same>>")]
    pub rounds: HashMap<Round, RoundVotes>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vote {
    #[serde(with = "As::<SerdeVoteType>")]
    pub vote_type: VoteType,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    #[serde(with = "As::<SerdeValue>")]
    pub value_id: Value,
    pub src_address: Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundVotes {
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub prevotes: VoteCount,
    pub precommits: VoteCount,
    #[serde(with = "As::<HashSet<SerdeVoteKeeperOutput>>")]
    pub emitted_outputs: HashSet<VoteKeeperOutput>,
    #[serde(with = "As::<HashMap<Same, Integer>>")]
    pub votes_addresses_weights: HashMap<Address, Weight>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteCount {
    #[serde(with = "As::<Integer>")]
    pub total_weight: Weight,
    #[serde(with = "As::<HashMap<SerdeValue, Integer>>")]
    pub values_weights: HashMap<Value, Weight>,
    pub votes_addresses: HashSet<Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub bookkeeper: Bookkeeper,
    #[serde(with = "As::<SerdeVoteKeeperOutput>")]
    pub last_emitted: VoteKeeperOutput,
    #[serde(with = "As::<SerdeWeightedVote>")]
    pub weighted_vote: WeightedVote,
}

use itf::de::{As, Integer, Same};
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::types::{Address, Height, NonNilValue, Round, Value, VoteType, Weight};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum WeightedVote {
    NoWeightedVote,

    #[serde(with = "As::<(Same, Integer, Integer)>")]
    WV(Vote, Weight, Round),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum VoteKeeperOutput {
    NoVKOutput,
    #[serde(with = "As::<Integer>")]
    PolkaAnyVKOutput(Round),
    #[serde(with = "As::<Integer>")]
    PolkaNilVKOutput(Round),
    #[serde(with = "As::<(Integer, Same)>")]
    PolkaValueVKOutput(Round, NonNilValue),
    #[serde(with = "As::<Integer>")]
    PrecommitAnyVKOutput(Round),
    #[serde(with = "As::<(Integer, Same)>")]
    PrecommitValueVKOutput(Round, NonNilValue),
    #[serde(with = "As::<Integer>")]
    SkipVKOutput(Round),
}

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
    pub vote_type: VoteType,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
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
    pub emitted_outputs: HashSet<VoteKeeperOutput>,
    #[serde(with = "As::<HashMap<Same, Integer>>")]
    pub votes_addresses_weights: HashMap<Address, Weight>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteCount {
    #[serde(with = "As::<Integer>")]
    pub total_weight: Weight,
    #[serde(with = "As::<HashMap<Same, Integer>>")]
    pub values_weights: HashMap<Value, Weight>,
    pub votes_addresses: HashSet<Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub bookkeeper: Bookkeeper,
    pub last_emitted: VoteKeeperOutput,
    pub weighted_vote: WeightedVote,
}

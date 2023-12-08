use itf::de::{As, Integer, Same};
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::types::{Address, Height, NonNilValue, Round, Value, Vote, Weight};

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
    NoVOputput,
    #[serde(with = "As::<Integer>")]
    PolkaAnyVOputput(Round),
    #[serde(with = "As::<Integer>")]
    PolkaNilVOputput(Round),
    #[serde(with = "As::<(Integer, Same)>")]
    PolkaValueVOputput(Round, NonNilValue),
    #[serde(with = "As::<Integer>")]
    PrecommitAnyVOputput(Round),
    #[serde(with = "As::<(Integer, Same)>")]
    PrecommitValueVOputput(Round, NonNilValue),
    #[serde(with = "As::<Integer>")]
    SkipVOputput(Round),
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

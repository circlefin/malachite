use itf::de::{As, Integer, Same};
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::types::{Address, Height, NonNilValue, Round, Value, Vote, Weight};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum WeightedVote {
    #[serde(rename = "NoWeightedVote")]
    NoVote,

    #[serde(rename = "WV")]
    #[serde(with = "As::<(Same, Integer, Integer)>")]
    Vote(Vote, Weight, Round),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "tag", content = "value")]
pub enum VoteKeeperOutput {
    #[serde(rename = "NoVKOutput")]
    NoOutput,

    #[serde(rename = "PolkaAnyVKOutput")]
    #[serde(with = "As::<Integer>")]
    PolkaAny(Round),

    #[serde(rename = "PolkaNilVKOutput")]
    #[serde(with = "As::<Integer>")]
    PolkaNil(Round),

    #[serde(rename = "PolkaValueVKOutput")]
    #[serde(with = "As::<(Integer, Same)>")]
    PolkaValue(Round, NonNilValue),

    #[serde(rename = "PrecommitAnyVKOutput")]
    #[serde(with = "As::<Integer>")]
    PrecommitAny(Round),

    #[serde(rename = "PrecommitValueVKOutput")]
    #[serde(with = "As::<(Integer, Same)>")]
    PrecommitValue(Round, NonNilValue),

    #[serde(rename = "SkipVKOutput")]
    #[serde(with = "As::<Integer>")]
    Skip(Round),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookkeeper {
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<HashMap<Same, Integer>>")]
    pub validator_set: HashMap<Address, Weight>,
    #[serde(with = "As::<HashMap<Integer, Same>>")]
    pub rounds: HashMap<Round, RoundVotes>,
}

impl Bookkeeper {
    pub fn total_weight(&self) -> Weight {
        self.validator_set.values().sum()
    }
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

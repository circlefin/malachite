use itf::de::{As, Integer, Same};
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

pub type Height = i64;
pub type Weight = i64;
pub type Round = i64;
pub type Address = String;
pub type Value = String;
pub type VoteType = String;

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
pub struct Vote {
    pub typ: VoteType,
    #[serde(with = "As::<Integer>")]
    pub height: Height,
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub value: Value,
    pub address: Address,
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
pub struct VoteKeeperOutput {
    #[serde(with = "As::<Integer>")]
    pub round: Round,
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct State {
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::bookkeeper")]
    pub bookkeeper: Bookkeeper,
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::lastEmitted")]
    pub last_emitted: VoteKeeperOutput,
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::weightedVote")]
    #[serde(with = "As::<(Same, Integer, Integer)>")]
    pub weighted_vote: (Vote, Weight, Round),
}

use itf::{ItfBigInt, ItfMap, ItfSet};
use serde::Deserialize;

pub type Height = ItfBigInt;
pub type Weight = ItfBigInt;
pub type Round = ItfBigInt;
pub type Address = String;
pub type Value = String;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookkeeper {
    pub height: Height,
    pub total_weight: Weight,
    pub rounds: ItfMap<Round, RoundVotes>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundVotes {
    pub height: Height,
    pub round: Round,
    pub prevotes: VoteCount,
    pub precommits: VoteCount,
    pub emitted_events: ItfSet<ExecutorEvent>,
    pub votes_addresses_weights: ItfMap<Address, Weight>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteCount {
    pub total_weight: Weight,
    pub values_weights: ItfMap<Value, Weight>,
    pub votes_addresses: ItfSet<Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
pub struct ExecutorEvent {
    pub round: Round,
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub bookkeeper: Bookkeeper,
    pub last_emitted: ExecutorEvent,
}

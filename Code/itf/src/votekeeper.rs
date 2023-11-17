use itf::{ItfBigInt, ItfMap, ItfSet, ItfTuple};
use serde::Deserialize;

pub type Height = ItfBigInt;
pub type Weight = ItfBigInt;
pub type Round = ItfBigInt;
pub type Address = String;
pub type Value = String;
pub type VoteType = String;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookkeeper {
    pub height: Height,
    pub current_round: Round,
    pub total_weight: Weight,
    pub rounds: ItfMap<Round, RoundVotes>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Vote {
    pub typ: VoteType,
    pub height: Height,
    pub round: Round,
    pub value: Value,
    pub address: Address,
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
pub struct State {
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::bookkeeper")]
    pub bookkeeper: Bookkeeper,
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::lastEmitted")]
    pub last_emitted: ExecutorEvent,
    #[serde(rename = "voteBookkeeperTest::voteBookkeeperSM::weightedVote")]
    pub weighted_vote: ItfTuple<(Vote, Weight)>,
}

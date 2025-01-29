use crate::deserializers as de;
use itf::de::{As, Integer};
use serde::Deserialize;
use std::collections::HashSet;

pub type Sequence = i64;
pub type Payload = String;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(tag = "tag")]
pub enum MessageType {
    #[serde(rename = "INIT")]
    Init,
    #[serde(rename = "DATA")]
    Data,
    #[serde(rename = "FIN")]
    Fin,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(with = "As::<Integer>")]
    sequence: Sequence,
    msg_type: MessageType,
    payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamState {
    pub fin_received: bool,
    #[serde(deserialize_with = "de::quint_option_message")]
    pub init_message: Option<Message>,
    #[serde(with = "As::<Integer>")]
    pub next_sequence: Sequence,
    #[serde(with = "As::<Integer>")]
    pub total_messages: i32,
    pub emitted: Vec<Message>,
    pub received: HashSet<Message>,
}

// StreamState is one state variable and State represents whole state machines state
// In this case they are equivalent
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct State {
    pub state: StreamState,
}

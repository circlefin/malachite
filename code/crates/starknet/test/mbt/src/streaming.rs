use crate::deserializers as de;
use itf::de::{As, Integer};
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashSet},
};

pub type Sequence = i64;
pub type Payload = String;

// This and buffer struct are defined just for testing purposes so they implement and contain
// only the necessary functions and fields (e.g. push, peek, pop functions are not implemented)
#[derive(Clone, Debug, Deserialize)]
pub struct BufferRecord(#[serde(with = "As::<Integer>")] pub Sequence, pub Message);

impl PartialEq for BufferRecord {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for BufferRecord {}

impl PartialOrd for BufferRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Buffer should act as MinHeap so the ordering is reversed
impl Ord for BufferRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Buffer(pub BinaryHeap<BufferRecord>);

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.0.iter().eq(other.0.iter())
    }
}

impl Eq for Buffer {}

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
    pub sequence: Sequence,
    pub msg_type: MessageType,
    pub payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamState {
    pub buffer: Buffer,
    #[serde(deserialize_with = "de::quint_option_message")]
    pub init_message: Option<Message>,
    pub received: HashSet<Message>,
    #[serde(with = "As::<Integer>")]
    pub next_sequence: Sequence,
    #[serde(with = "As::<Integer>")]
    pub total_messages: i32,
    pub fin_received: bool,
    pub emitted: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub state: StreamState,
    #[serde(deserialize_with = "de::quint_option_message")]
    pub incoming_message: Option<Message>,
}

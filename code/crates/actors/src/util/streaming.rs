use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use derive_where::derive_where;
use libp2p::PeerId;
use malachite_common::{Context, ProposalPart, Round};

pub type StreamId = u64;
pub type Sequence = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamMessage<T> {
    /// Receivers identify streams by (sender, stream_id).
    /// This means each node can allocate stream_ids independently
    /// and that many streams can be sent on a single network topic.
    pub stream_id: StreamId,

    /// Identifies the sequence of each message in the stream starting from 0.
    pub sequence: Sequence,

    /// The content of this stream message
    pub content: StreamContent<T>,
}

impl<T> StreamMessage<T> {
    pub fn new(stream_id: StreamId, sequence: Sequence, content: StreamContent<T>) -> Self {
        Self {
            stream_id,
            sequence,
            content,
        }
    }

    pub fn is_first(&self) -> bool {
        self.sequence == 0
    }

    pub fn is_fin(&self) -> bool {
        self.content.is_fin()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamContent<T> {
    /// Serialized content.
    Data(T),

    /// Fin must be set to true.
    Fin(bool),
}

impl<T> StreamContent<T> {
    pub fn as_data(&self) -> Option<&T> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }

    pub fn into_data(self) -> Option<T> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }

    pub fn is_fin(&self) -> bool {
        matches!(self, Self::Fin(true))
    }
}

struct MinSeq<T>(StreamMessage<T>);

impl<T> PartialEq for MinSeq<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.sequence == other.0.sequence
    }
}

impl<T> Eq for MinSeq<T> {}

impl<T> Ord for MinSeq<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.sequence.cmp(&self.0.sequence)
    }
}

impl<T> PartialOrd for MinSeq<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive_where(Default)]
struct StreamState<Ctx, T>
where
    Ctx: Context,
{
    buffer: BinaryHeap<MinSeq<T>>,
    init_info: Option<(Ctx::Height, Round)>,
    next_sequence: Sequence,
    total_messages: usize,
    fin_received: bool,
}

#[derive_where(Default)]
pub struct PartStreamsMap<Ctx>
where
    Ctx: Context,
{
    streams: BTreeMap<(PeerId, StreamId), StreamState<Ctx, Ctx::ProposalPart>>,
}

impl<Ctx> PartStreamsMap<Ctx>
where
    Ctx: Context,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        peer_id: PeerId,
        msg: StreamMessage<Ctx::ProposalPart>,
    ) -> Option<(Ctx::Height, Round, Vec<Ctx::ProposalPart>)> {
        let state = self.streams.entry((peer_id, msg.stream_id)).or_default();

        if msg.is_first() {
            return Self::insert_first(state, msg);
        }

        if msg.is_fin() {
            state.fin_received = true;
            state.total_messages = msg.sequence as usize + 1;
        }

        state.buffer.push(MinSeq(msg));

        let mut to_emit = vec![];
        Self::emit_eligible_messages(state, &mut to_emit);

        if to_emit.is_empty() {
            return None;
        }

        let (height, round) = state.init_info.unwrap();
        Some((height, round, to_emit))
    }

    fn emit(
        state: &mut StreamState<Ctx, Ctx::ProposalPart>,
        msg: StreamMessage<Ctx::ProposalPart>,
        to_emit: &mut Vec<Ctx::ProposalPart>,
    ) {
        if let Some(data) = msg.content.into_data() {
            to_emit.push(data);
        }

        state.next_sequence = msg.sequence + 1;
    }

    fn emit_eligible_messages(
        state: &mut StreamState<Ctx, Ctx::ProposalPart>,
        to_emit: &mut Vec<Ctx::ProposalPart>,
    ) {
        while !state.buffer.is_empty() {
            let MinSeq(msg) = state.buffer.pop().unwrap();
            if msg.sequence == state.next_sequence {
                Self::emit(state, msg, to_emit);
            } else {
                state.buffer.push(MinSeq(msg));
                break;
            }
        }
    }

    fn insert_first(
        state: &mut StreamState<Ctx, Ctx::ProposalPart>,
        msg: StreamMessage<Ctx::ProposalPart>,
    ) -> Option<(Ctx::Height, Round, Vec<Ctx::ProposalPart>)> {
        state.init_info = msg.content.as_data().and_then(|p| p.info());

        let mut to_emit = vec![];
        Self::emit(state, msg, &mut to_emit);
        Self::emit_eligible_messages(state, &mut to_emit);

        let (height, round) = state.init_info.unwrap();
        Some((height, round, to_emit))
    }
}

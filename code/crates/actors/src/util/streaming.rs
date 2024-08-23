use std::collections::BTreeMap;
use std::ops::Range;

use derive_where::derive_where;
use libp2p::PeerId;
use malachite_common::{Context, ProposalPart, Round};

pub type StreamId = u64;
pub type Sequence = u64;

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

#[derive_where(Default)]
struct StreamState<Ctx, T>
where
    Ctx: Context,
{
    info: Option<(Ctx::Height, Round)>,
    messages: Vec<StreamMessage<T>>,
    has_first: bool,
    fin_sequence: Option<u64>,
}

impl<Ctx, T> StreamState<Ctx, T>
where
    Ctx: Context,
{
    fn into_data(self) -> Vec<T> {
        self.messages
            .into_iter()
            .filter_map(|msg| msg.content.into_data())
            .collect()
    }

    fn drain_data(&mut self, range: Range<usize>) -> Vec<T> {
        self.messages
            .drain(range)
            .filter_map(|msg| msg.content.into_data())
            .collect()
    }
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
        let is_first = msg.is_first();
        let is_fin = msg.is_fin();
        let stream_id = msg.stream_id;
        let msg_seq = msg.sequence;
        let info = msg.content.as_data().and_then(|part| part.info());

        tracing::debug!(
            "ProposalPart: first={is_first} fin={is_fin} stream={stream_id} seq={msg_seq} info={info:?}"
        );

        let state = self.streams.entry((peer_id, stream_id)).or_default();
        state.messages.push(msg);
        state.messages.sort_unstable_by_key(|msg| msg.sequence);

        // Init
        // Send init and all subsequent messages without gaps in sequence number
        if is_first {
            state.has_first = true;
            state.info = info;

            let (height, round) = state.info.unwrap();
            let range = subsequent_seqs(&state.messages, 0);
            tracing::debug!("Init: range={range:?}");
            let messages = state.drain_data(range);
            tracing::debug!("Init: messages={}", messages.len());
            return Some((height, round, messages));
        }

        // Fin
        // If have all messages: then return all messages
        // Otherwise, update fin_sequence
        if is_fin {
            if has_all_messages(state) {
                let state = self.streams.remove(&(peer_id, stream_id)).unwrap();
                let (height, round) = state.info.unwrap();
                let messages = state.into_data();
                tracing::debug!("Fin: messages={}", messages.len());
                return Some((height, round, messages));
            } else {
                tracing::debug!("Fin: sequence={msg_seq}");
                state.fin_sequence = Some(msg_seq);
            }
        }

        // Normal
        // If have all messages: then return all messages
        // Otherwise:
        //   - If have init then send init and all subsequent messages without gaps in sequence number
        //   - Otherwise, return None
        if has_all_messages(state) {
            let state = self.streams.remove(&(peer_id, stream_id)).unwrap();
            let (height, round) = state.info.unwrap();
            let messages = state.into_data();
            tracing::debug!("Normal: messages={}", messages.len());
            return Some((height, round, messages));
        }

        if state.has_first {
            let range = subsequent_seqs(&state.messages, state.messages[0].sequence);
            tracing::debug!("Normal: range={range:?}");
            let (height, round) = state.info.unwrap();
            let messages = state.drain_data(range);
            tracing::debug!("Normal: messages={}", messages.len());
            if !messages.is_empty() {
                return Some((height, round, messages));
            }
        }

        tracing::debug!("Normal: None");
        None
    }
}

fn has_all_messages<Ctx: Context, T>(state: &StreamState<Ctx, T>) -> bool {
    if !state.has_first {
        return false;
    }

    tracing::debug!("{} == {:?} + 1", state.messages.len(), state.fin_sequence);

    if let Some(fin_sequence) = state.fin_sequence {
        state.messages.len() as u64 == fin_sequence + 1
    } else {
        false
    }
}

fn subsequent_seqs<T>(messages: &[StreamMessage<T>], start: u64) -> Range<usize> {
    let mut sequence = start;
    for msg in messages {
        if msg.sequence == sequence {
            sequence += 1;
        } else {
            break;
        }
    }
    0..(sequence - start) as usize
}

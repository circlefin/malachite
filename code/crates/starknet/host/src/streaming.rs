use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashSet};

use derive_where::derive_where;

use malachite_consensus::PeerId;
use malachite_core_types::Round;
use malachite_engine::util::streaming::{Sequence, StreamId, StreamMessage};

use crate::types::{Address, Height, ProposalInit, ProposalPart};

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

struct MinHeap<T>(BinaryHeap<MinSeq<T>>);

impl<T> Default for MinHeap<T> {
    fn default() -> Self {
        Self(BinaryHeap::new())
    }
}

impl<T> MinHeap<T> {
    fn push(&mut self, msg: StreamMessage<T>) {
        self.0.push(MinSeq(msg));
    }

    fn pop(&mut self) -> Option<StreamMessage<T>> {
        self.0.pop().map(|msg| msg.0)
    }

    fn peek(&self) -> Option<&StreamMessage<T>> {
        self.0.peek().map(|msg| &msg.0)
    }
}

#[derive_where(Default)]
struct StreamState<T> {
    buffer: MinHeap<T>,
    init_info: Option<ProposalInit>,
    seen_sequences: HashSet<Sequence>,
    next_sequence: Sequence,
    total_messages: usize,
    fin_received: bool,
    emitted_messages: usize,
}

impl<T> StreamState<T> {
    fn has_emitted_all_messages(&self) -> bool {
        self.fin_received && self.emitted_messages == self.total_messages
    }

    fn emit(&mut self, msg: StreamMessage<T>, to_emit: &mut Vec<T>) {
        if let Some(data) = msg.content.into_data() {
            to_emit.push(data);
        }

        self.next_sequence = msg.sequence + 1;
        self.emitted_messages += 1;
    }

    fn emit_eligible_messages(&mut self, to_emit: &mut Vec<T>) {
        while let Some(msg) = self.buffer.peek() {
            if msg.sequence == self.next_sequence {
                let msg = self.buffer.pop().expect("peeked element should exist");
                self.emit(msg, to_emit);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalParts {
    pub height: Height,
    pub round: Round,
    pub proposer: Address,
    pub parts: Vec<ProposalPart>,
}

#[derive(Default)]
pub struct PartStreamsMap {
    streams: BTreeMap<(PeerId, StreamId), StreamState<ProposalPart>>,
}

impl PartStreamsMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        peer_id: PeerId,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<ProposalParts> {
        let stream_id = msg.stream_id;
        let state = self.streams.entry((peer_id, stream_id)).or_default();

        if !state.seen_sequences.insert(msg.sequence) {
            // We have already seen a message with this sequence number.
            return None;
        }

        let result = if msg.is_first() {
            Self::insert_first(state, msg)
        } else {
            Self::insert_other(state, msg)
        };

        if state.has_emitted_all_messages() {
            self.streams.remove(&(peer_id, stream_id));
        }

        result
    }

    fn insert_first(
        state: &mut StreamState<ProposalPart>,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<ProposalParts> {
        state.init_info = msg.content.as_data().and_then(|p| p.as_init()).cloned();

        let mut to_emit = Vec::with_capacity(1);
        state.emit(msg, &mut to_emit);
        state.emit_eligible_messages(&mut to_emit);

        let init_info = state.init_info.as_ref().unwrap();

        Some(ProposalParts {
            height: init_info.height,
            round: init_info.proposal_round,
            proposer: init_info.proposer.clone(),
            parts: to_emit,
        })
    }

    fn insert_other(
        state: &mut StreamState<ProposalPart>,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<ProposalParts> {
        if msg.is_fin() {
            state.fin_received = true;
            state.total_messages = msg.sequence as usize + 1;
        }

        state.buffer.push(msg);

        let mut to_emit = vec![];
        state.emit_eligible_messages(&mut to_emit);

        if to_emit.is_empty() {
            return None;
        }

        let init_info = state.init_info.as_ref().unwrap();

        Some(ProposalParts {
            height: init_info.height,
            round: init_info.proposal_round,
            proposer: init_info.proposer.clone(),
            parts: to_emit,
        })
    }
}

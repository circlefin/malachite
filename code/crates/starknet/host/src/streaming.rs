// When inserting part in a map, stream tries to connect all received parts in the right order,
// starting from beginning and emits parts sequence chunks  when it succeeds
// If it can't connect part, it buffers it
// E.g. buffered: 1 3 7
// part 0 (first) arrives -> 0 and 1 are emitted
// 4 arrives -> gets buffered
// 2 arrives -> 2, 3 and 4 are emitted

// `insert` returns connected sequence of parts if any is emitted

// Emitted parts are stored and simulated (if it is tx)
// (this is done inside `actor::on_received_proposal_part`)
// When finish part is stored, proposal value is built from all of them
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashSet};
use std::fmt::Debug;

use derive_where::derive_where;

use malachitebft_core_consensus::PeerId;
use malachitebft_core_types::Round;
use malachitebft_engine::util::streaming::{Sequence, StreamId, StreamMessage};

use crate::types::{Address, Height, ProposalInit, ProposalPart};

/// Wraps a [`StreamMessage`] to implement custom ordering for a [`BinaryHeap`].
///
/// The default `BinaryHeap` is a max-heap, so we reverse the ordering
/// by implementing `Ord` in reverse to make it a min-heap, which suits the purpose of efficiently
/// providing available proposal part with smallest sequence number.
#[derive(Debug)]
pub struct MinSeq<T>(pub StreamMessage<T>);

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

#[derive(Debug)]
pub struct MinHeap<T>(pub BinaryHeap<MinSeq<T>>);

impl<T> Default for MinHeap<T> {
    fn default() -> Self {
        Self(BinaryHeap::new())
    }
}

impl<T> MinHeap<T> {
    pub fn push(&mut self, msg: StreamMessage<T>) {
        self.0.push(MinSeq(msg));
    }

    pub fn pop(&mut self) -> Option<StreamMessage<T>> {
        self.0.pop().map(|msg| msg.0)
    }

    fn peek(&self) -> Option<&StreamMessage<T>> {
        self.0.peek().map(|msg| &msg.0)
    }
}
#[derive(Debug)]
#[derive_where(Default)]
pub struct StreamState<T> {
    pub buffer: MinHeap<T>,
    pub init_info: Option<ProposalInit>,
    pub seen_sequences: HashSet<Sequence>,
    pub next_sequence: Sequence,
    pub total_messages: usize,
    pub fin_received: bool,
    pub emitted_messages: usize,
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

    // Emits all buffered successive parts if they are next in sequence
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
    pub streams: BTreeMap<(PeerId, StreamId), StreamState<ProposalPart>>,
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
        let stream_id = msg.stream_id.clone();
        let state = self
            .streams
            .entry((peer_id, stream_id.clone()))
            .or_default();

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
            round: init_info.round,
            proposer: init_info.proposer,
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
            round: init_info.round,
            proposer: init_info.proposer,
            parts: to_emit,
        })
    }
}

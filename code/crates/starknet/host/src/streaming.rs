use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use derive_where::derive_where;

use malachite_actors::util::streaming::{Sequence, StreamId, StreamMessage};
use malachite_common::Round;
use malachite_gossip_mempool::PeerId;
use malachite_starknet_p2p_types::{Height, ProposalInit, ProposalPart};

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
    next_sequence: Sequence,
    total_messages: usize,
    fin_received: bool,
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
    ) -> Option<(Height, Round, Vec<ProposalPart>)> {
        let state = self.streams.entry((peer_id, msg.stream_id)).or_default();

        if msg.is_first() {
            return Self::insert_first(state, msg);
        }

        if msg.is_fin() {
            state.fin_received = true;
            state.total_messages = msg.sequence as usize + 1;
        }

        state.buffer.push(msg);

        let mut to_emit = vec![];
        Self::emit_eligible_messages(state, &mut to_emit);

        if to_emit.is_empty() {
            return None;
        }

        let init_info = state.init_info.as_ref().unwrap();
        Some((init_info.block_number, init_info.proposal_round, to_emit))
    }

    fn emit(
        state: &mut StreamState<ProposalPart>,
        msg: StreamMessage<ProposalPart>,
        to_emit: &mut Vec<ProposalPart>,
    ) {
        if let Some(data) = msg.content.into_data() {
            to_emit.push(data);
        }

        state.next_sequence = msg.sequence + 1;
    }

    fn emit_eligible_messages(
        state: &mut StreamState<ProposalPart>,
        to_emit: &mut Vec<ProposalPart>,
    ) {
        while let Some(msg) = state.buffer.peek() {
            if msg.sequence == state.next_sequence {
                let msg = state.buffer.pop().expect("peeked element should exist");
                Self::emit(state, msg, to_emit);
            } else {
                break;
            }
        }
    }

    fn insert_first(
        state: &mut StreamState<ProposalPart>,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<(Height, Round, Vec<ProposalPart>)> {
        state.init_info = msg.content.as_data().and_then(|p| p.as_init()).cloned();

        let mut to_emit = vec![];
        Self::emit(state, msg, &mut to_emit);
        Self::emit_eligible_messages(state, &mut to_emit);

        let init_info = state.init_info.as_ref().unwrap();
        Some((init_info.block_number, init_info.proposal_round, to_emit))
    }
}

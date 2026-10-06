use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashSet};

use malachitebft_app_channel::app::consensus::PeerId;
use malachitebft_app_channel::app::streaming::{Sequence, StreamId, StreamMessage};
use malachitebft_app_channel::app::types::core::Round;
use malachitebft_test::{Address, Height, ProposalFin, ProposalInit, ProposalPart};

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

    fn len(&self) -> usize {
        self.0.len()
    }

    fn drain(&mut self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.0.len());
        while let Some(MinSeq(msg)) = self.0.pop() {
            if let Some(data) = msg.content.into_data() {
                vec.push(data);
            }
        }
        vec
    }
}

#[derive(Default)]
struct StreamState {
    buffer: MinHeap<ProposalPart>,
    init_info: Option<ProposalInit>,
    seen_sequences: HashSet<Sequence>,
    total_messages: usize,
    fin_received: bool,
}

impl StreamState {
    fn is_done(&self) -> bool {
        self.init_info.is_some() && self.fin_received && self.buffer.len() == self.total_messages
    }

    fn insert(&mut self, msg: StreamMessage<ProposalPart>) -> Option<ProposalParts> {
        if msg.is_first() {
            self.init_info = msg.content.as_data().and_then(|p| p.as_init()).cloned();
        }

        if msg.is_fin() {
            self.fin_received = true;
            self.total_messages = msg.sequence as usize + 1;
        }

        self.buffer.push(msg);

        if self.is_done() {
            let init_info = self.init_info.take()?;

            Some(ProposalParts {
                height: init_info.height,
                round: init_info.round,
                proposer: init_info.proposer,
                parts: self.buffer.drain(),
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProposalParts {
    pub height: Height,
    pub round: Round,
    pub proposer: Address,
    pub parts: Vec<ProposalPart>,
}

impl ProposalParts {
    pub fn init(&self) -> Option<&ProposalInit> {
        self.parts.iter().find_map(|p| p.as_init())
    }

    pub fn fin(&self) -> Option<&ProposalFin> {
        self.parts.iter().find_map(|p| p.as_fin())
    }
}

#[derive(Default)]
pub struct PartStreamsMap {
    /// Keyed by the publisher of the stream: the parts of one proposal can
    /// arrive through different peers and must land in the same entry.
    streams: BTreeMap<(PeerId, StreamId), StreamState>,
}

impl PartStreamsMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a proposal part.
    ///
    /// `delivered_by` is the peer the part arrived from. `published_by` is the
    /// publisher declared in the message, used to group parts of one proposal;
    /// it falls back to `delivered_by` for transports that carry no publisher.
    pub fn insert(
        &mut self,
        delivered_by: PeerId,
        published_by: Option<PeerId>,
        msg: StreamMessage<ProposalPart>,
    ) -> Option<ProposalParts> {
        let stream_id = msg.stream_id.clone();
        let publisher = published_by.unwrap_or(delivered_by);

        let state = self
            .streams
            .entry((publisher, stream_id.clone()))
            .or_default();

        if !state.seen_sequences.insert(msg.sequence) {
            // We have already seen a message with this sequence number.
            return None;
        }

        let result = state.insert(msg);

        if state.is_done() {
            self.streams.remove(&(publisher, stream_id));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use malachitebft_app_channel::app::streaming::StreamContent;
    use malachitebft_app_channel::app::types::core::Round;
    use malachitebft_test::{Address, Height, ProposalInit, ProposalPart};

    use super::*;

    fn init_part() -> ProposalPart {
        ProposalPart::Init(ProposalInit::new(
            Height::new(1),
            Round::new(0),
            Round::Nil,
            Address::new([0xa; 20]),
        ))
    }

    fn data_message(stream_id: &StreamId, sequence: Sequence) -> StreamMessage<ProposalPart> {
        let part = if sequence == 0 {
            init_part()
        } else {
            ProposalPart::Data(malachitebft_test::ProposalData::new(sequence))
        };

        StreamMessage::new(stream_id.clone(), sequence, StreamContent::Data(part))
    }

    fn fin_message(stream_id: &StreamId, sequence: Sequence) -> StreamMessage<ProposalPart> {
        StreamMessage::new(stream_id.clone(), sequence, StreamContent::Fin)
    }

    #[test]
    fn parts_relayed_by_different_peers_reassemble() {
        let publisher = PeerId::random();
        let first_relay = PeerId::random();
        let second_relay = PeerId::random();
        let stream_id = StreamId::new(vec![1].into());

        let mut map = PartStreamsMap::new();

        assert!(map
            .insert(first_relay, Some(publisher), data_message(&stream_id, 0))
            .is_none());
        assert!(map
            .insert(second_relay, Some(publisher), data_message(&stream_id, 1))
            .is_none());

        let parts = map
            .insert(first_relay, Some(publisher), fin_message(&stream_id, 2))
            .expect("parts published by one peer reassemble whichever peer relays them");

        assert_eq!(parts.height, Height::new(1));
        assert_eq!(parts.parts.len(), 2);
    }

    #[test]
    fn parts_from_distinct_publishers_stay_separate() {
        let publisher = PeerId::random();
        let other_publisher = PeerId::random();
        let relay = PeerId::random();
        let stream_id = StreamId::new(vec![1].into());

        let mut map = PartStreamsMap::new();

        assert!(map
            .insert(relay, Some(publisher), data_message(&stream_id, 0))
            .is_none());

        // Same stream id, different publisher: must not join the stream above.
        assert!(map
            .insert(relay, Some(other_publisher), fin_message(&stream_id, 1))
            .is_none());

        let parts = map
            .insert(relay, Some(publisher), fin_message(&stream_id, 1))
            .expect("the publisher's own stream completes");

        assert_eq!(parts.parts.len(), 1);
    }

    #[test]
    fn parts_without_a_publisher_group_by_delivering_peer() {
        let sender = PeerId::random();
        let stream_id = StreamId::new(vec![1].into());

        let mut map = PartStreamsMap::new();

        assert!(map
            .insert(sender, None, data_message(&stream_id, 0))
            .is_none());

        let parts = map
            .insert(sender, None, fin_message(&stream_id, 1))
            .expect("a stream with no declared publisher still completes");

        assert_eq!(parts.parts.len(), 1);
    }
}

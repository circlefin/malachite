use std::collections::BTreeMap;
use std::sync::Arc;

use derive_where::derive_where;

use malachitebft_core_types::{Context, Round, ValueId};
use malachitebft_engine::util::streaming::StreamId;

// This is a temporary store implementation for proposal parts
//
// TODO: Add Address to key
// NOTE: Not sure if this is required as consensus should verify that only the parts signed by the proposer for
//       the height and round should be forwarded here (see the TODOs in consensus)

type Key<Height> = (StreamId, Height, Round);

/// Stores proposal parts for a given stream, height, and round.
/// `value_id` is the value id of the proposal as computed by the proposer. It is also included in one of the parts but stored here for convenience.
/// `parts` is a list of `ProposalPart`s, ordered by the sequence of the `StreamMessage` that delivered them.
#[derive_where(Clone, Debug, Default)]
pub struct Entry<Ctx: Context> {
    pub value_id: Option<ValueId<Ctx>>,
    pub parts: Vec<Arc<<Ctx as Context>::ProposalPart>>,
}
type Store<Ctx> = BTreeMap<Key<<Ctx as Context>::Height>, Entry<Ctx>>;

#[derive_where(Clone, Debug)]
pub struct PartStore<Ctx: Context> {
    store: Store<Ctx>,
}

impl<Ctx: Context> Default for PartStore<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx: Context> PartStore<Ctx> {
    pub fn new() -> Self {
        Self {
            store: Default::default(),
        }
    }

    /// Return all the parts for the given `stream_id`, `height` and `round`.
    /// Parts are already sorted by sequence in ascending order.
    pub fn all_parts_by_stream_id(
        &self,
        stream_id: StreamId,
        height: Ctx::Height,
        round: Round,
    ) -> Vec<Arc<Ctx::ProposalPart>> {
        self.store
            .get(&(stream_id, height, round))
            .map(|entry| entry.parts.clone())
            .unwrap_or_default()
    }

    /// Return all the parts for the given `value_id`. If multiple entries with same `value_id` are present, the parts of the first one are returned.
    /// Parts are already sorted by sequence in ascending order.
    pub fn all_parts_by_value_id(&self, value_id: &ValueId<Ctx>) -> Vec<Arc<Ctx::ProposalPart>> {
        self.store
            .values()
            .find(|entry| entry.value_id.as_ref() == Some(value_id))
            .map(|entry| entry.parts.clone())
            .unwrap_or_default()
    }

    /// Store a part for the given `stream_id`, `height` and `round`.
    /// The part is added to the end of the list of parts and is for the next sequence number after the last part.
    pub fn store(
        &mut self,
        stream_id: &StreamId,
        height: Ctx::Height,
        round: Round,
        proposal_part: Ctx::ProposalPart,
    ) {
        let existing = self
            .store
            .entry((stream_id.clone(), height, round))
            .or_default();
        existing.parts.push(Arc::new(proposal_part));
    }

    /// Store the `value_id` of the proposal, as computed by the proposer, for the given `stream_id`, `height` and `round`.
    pub fn store_value_id(
        &mut self,
        stream_id: &StreamId,
        height: Ctx::Height,
        round: Round,
        value_id: ValueId<Ctx>,
    ) {
        let existing = self
            .store
            .entry((stream_id.clone(), height, round))
            .or_default();
        existing.value_id = Some(value_id);
    }

    /// Prune the parts for all heights lower than `min_height`.
    /// This is used to prune the parts from the store when a min_height has been finalized.
    /// Parts for higher heights may be present if the node is lagging and are kept.
    pub fn prune(&mut self, min_height: Ctx::Height) {
        self.store.retain(|(_, height, _), _| *height >= min_height);
    }

    /// Return the number of blocks in the store.
    pub fn blocks_count(&self) -> usize {
        self.store.len()
    }
}

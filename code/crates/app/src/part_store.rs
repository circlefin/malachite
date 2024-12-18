use std::collections::BTreeMap;
use std::sync::Arc;

use derive_where::derive_where;

use malachitebft_core_types::{Context, Round, ValueId};

// This is a temporary store implementation for proposal parts
//
// TODO: Add Address to key
// NOTE: Not sure if this is required as consensus should verify that only the parts signed by the proposer for
//       the height and round should be forwarded here (see the TODOs in consensus)

type Key<Height> = (Height, Round);

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

    /// Return all the parts for the given height and round, sorted by sequence in ascending order
    pub fn all_parts(&self, height: Ctx::Height, round: Round) -> Vec<Arc<Ctx::ProposalPart>> {
        self.store
            .get(&(height, round))
            .map(|entry| &entry.parts)
            .cloned()
            .unwrap_or_default()
    }

    pub fn store(&mut self, height: Ctx::Height, round: Round, proposal_part: Ctx::ProposalPart) {
        let existing = self.store.entry((height, round)).or_default();

        existing.parts.push(Arc::new(proposal_part));
    }

    pub fn store_value_id(&mut self, height: Ctx::Height, round: Round, value_id: ValueId<Ctx>) {
        let existing = self.store.entry((height, round)).or_default();

        existing.value_id = Some(value_id);
    }

    pub fn prune(&mut self, min_height: Ctx::Height) {
        self.store.retain(|(height, _), _| *height >= min_height);
    }

    pub fn blocks_count(&self) -> usize {
        self.store.len()
    }

    /// Return all the parts for the given height and round, sorted by sequence in ascending order
    pub fn all_parts_by_value_id(&self, value_id: &ValueId<Ctx>) -> Vec<Arc<Ctx::ProposalPart>> {
        for entry in self.store.values() {
            if let Some(ref id) = entry.value_id {
                if value_id == id {
                    return entry.parts.clone();
                }
            }
        }

        vec![]
    }
}

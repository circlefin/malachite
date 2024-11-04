use std::collections::BTreeMap;
use std::sync::Arc;

use derive_where::derive_where;

use malachite_common::{Context, Round};

pub type Sequence = u64;

// This is a temporary store implementation for proposal parts
//
// TODO:
// - [ ] add Address to key
//       note: not sure if this is required as consensus should verify that only the parts signed by the proposer for
//             the height and round should be forwarded here (see the TODOs in consensus)

type Key<Height> = (Height, Round);
type Store<Ctx> = BTreeMap<Key<<Ctx as Context>::Height>, Vec<Arc<<Ctx as Context>::ProposalPart>>>;

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
            .cloned()
            .unwrap_or_default()
    }

    pub fn store(&mut self, height: Ctx::Height, round: Round, proposal_part: Ctx::ProposalPart) {
        self.store
            .entry((height, round))
            .or_default()
            .push(Arc::new(proposal_part));
    }

    pub fn prune(&mut self, min_height: Ctx::Height) {
        self.store.retain(|(height, _), _| *height >= min_height);
    }

    pub fn blocks_count(&self) -> usize {
        self.store.len()
    }
}

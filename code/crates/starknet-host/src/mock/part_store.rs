use std::collections::BTreeMap;
use std::sync::Arc;

use derive_where::derive_where;

use malachite_common::{Context, Round};
use parking_lot::RwLock;

pub type Sequence = u64;

// This is a temporary store implementation for block parts
//
// TODO-s:
// - [x] make it context generic
// - [ ] add Address to key
//       note: not sure if this is required as consensus should verify that only the parts signed by the proposer for
//             the height and round should be forwarded here (see the TODOs in consensus)

type Key<Height> = (Height, Round, Sequence);
type Store<Ctx> = BTreeMap<Key<<Ctx as Context>::Height>, Arc<<Ctx as Context>::BlockPart>>;

#[derive_where(Clone, Debug)]
pub struct PartStore<Ctx: Context> {
    map: Arc<RwLock<Store<Ctx>>>,
}

impl<Ctx: Context> Default for PartStore<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx: Context> PartStore<Ctx> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub fn get(
        &self,
        height: Ctx::Height,
        round: Round,
        sequence: Sequence,
    ) -> Option<Arc<Ctx::BlockPart>> {
        self.map.read().get(&(height, round, sequence)).cloned()
    }

    pub fn all_parts(&self, height: Ctx::Height, round: Round) -> Vec<Arc<Ctx::BlockPart>> {
        use itertools::Itertools;
        use malachite_common::BlockPart;

        self.map
            .read()
            .iter()
            .filter(|((h, r, _), _)| *h == height && *r == round)
            .map(|(_, b)| b)
            .cloned()
            .sorted_by_key(|b| std::cmp::Reverse(b.sequence()))
            .collect()
    }

    pub fn store(&self, block_part: Ctx::BlockPart) {
        use malachite_common::BlockPart;

        let height = block_part.height();
        let round = block_part.round();
        let sequence = block_part.sequence();

        self.map
            .write()
            .entry((height, round, sequence))
            .or_insert(Arc::new(block_part));
    }

    pub fn prune(&self, min_height: Ctx::Height) {
        self.map
            .write()
            .retain(|(height, _, _), _| *height >= min_height);
    }
}

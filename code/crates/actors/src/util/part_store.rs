use std::collections::BTreeMap;

use derive_where::derive_where;

use malachite_common::{Context, Round};

// This is a temporary store implementation for block parts
//
// TODO-s:
// - [x] make it context generic
// - [ ] add Address to key
//       note: not sure if this is required as consensus should verify that only the parts signed by the proposer for
//             the height and round should be forwarded here (see the TODOs in consensus)
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct PartStore<Ctx: Context> {
    pub map: BTreeMap<(Ctx::Height, Round, u64), Ctx::BlockPart>,
}

impl<Ctx: Context> Default for PartStore<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx: Context> PartStore<Ctx> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    pub fn get(&self, height: Ctx::Height, round: Round, sequence: u64) -> Option<&Ctx::BlockPart> {
        self.map.get(&(height, round, sequence))
    }

    pub fn all_parts(&self, height: Ctx::Height, round: Round) -> Vec<&Ctx::BlockPart> {
        use malachite_common::BlockPart;

        let mut block_parts: Vec<_> = self
            .map
            .iter()
            .filter(|((h, r, _), _)| *h == height && *r == round)
            .map(|(_, b)| b)
            .collect();

        block_parts.sort_by_key(|b| std::cmp::Reverse(b.sequence()));
        block_parts
    }

    pub fn store(&mut self, block_part: Ctx::BlockPart) {
        use malachite_common::BlockPart;

        let height = block_part.height();
        let round = block_part.round();
        let sequence = block_part.sequence();

        self.map
            .entry((height, round, sequence))
            .or_insert(block_part);
    }
}

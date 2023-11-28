use alloc::collections::BTreeMap;

use malachite_common::{Context, Proposal, Value};
use malachite_common::{Round, ValueId};

/// Stores proposals at each round, indexed by their value id.
pub struct Proposals<Ctx>
where
    Ctx: Context,
{
    // NOTE: We have to use a nested map instead of just one with tuple keys
    // otherwise we would have to clone the value id to call `get`.
    pub(crate) proposals: BTreeMap<Round, BTreeMap<ValueId<Ctx>, Ctx::Proposal>>,
}

impl<Ctx> Proposals<Ctx>
where
    Ctx: Context,
{
    pub fn new() -> Self {
        Self {
            proposals: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, proposal: Ctx::Proposal) {
        let round = proposal.round();
        let value_id = proposal.value().id();

        self.proposals
            .entry(round)
            .or_default()
            .insert(value_id, proposal);
    }

    pub fn get(&self, round: Round, value_id: &ValueId<Ctx>) -> Option<&Ctx::Proposal> {
        self.proposals
            .get(&round)
            .and_then(|proposals| proposals.get(value_id))
    }
}

impl<Ctx> Default for Proposals<Ctx>
where
    Ctx: Context,
{
    fn default() -> Self {
        Self::new()
    }
}

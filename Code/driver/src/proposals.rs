use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use malachite_common::ValueId;
use malachite_common::{Context, Proposal, Value};

/// Stores proposals at each round, indexed by their value id.
pub struct Proposals<Ctx>
where
    Ctx: Context,
{
    pub(crate) proposals: BTreeMap<ValueId<Ctx>, Vec<Ctx::Proposal>>,
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
        let value_id = proposal.value().id();
        self.proposals.entry(value_id).or_default().push(proposal);
    }

    pub fn find(
        &self,
        value_id: &ValueId<Ctx>,
        p: impl Fn(&Ctx::Proposal) -> bool,
    ) -> Option<&Ctx::Proposal> {
        self.proposals
            .get(value_id)
            .and_then(|proposals| proposals.iter().find(|proposal| p(proposal)))
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

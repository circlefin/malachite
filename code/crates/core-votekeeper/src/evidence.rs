//! Evidence of equivocation.

use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};

use derive_where::derive_where;

use malachite_core_types::{Context, SignedVote, Vote};

/// Keeps track of evidence of equivocation.
#[derive_where(Clone, Debug, Default)]
pub struct EvidenceMap<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::type_complexity)]
    map: BTreeMap<Ctx::Address, Vec<(SignedVote<Ctx>, SignedVote<Ctx>)>>,
}

impl<Ctx> EvidenceMap<Ctx>
where
    Ctx: Context,
{
    /// Create a new `EvidenceMap` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether or not there is any evidence of equivocation.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return the evidence of equivocation for a given address, if any.
    pub fn get(&self, address: &Ctx::Address) -> Option<&Vec<(SignedVote<Ctx>, SignedVote<Ctx>)>> {
        self.map.get(address)
    }

    /// Add evidence of equivocation.
    pub fn add(&mut self, existing: SignedVote<Ctx>, vote: SignedVote<Ctx>) {
        debug_assert_eq!(existing.validator_address(), vote.validator_address());

        if let Some(evidence) = self.map.get_mut(vote.validator_address()) {
            evidence.push((existing, vote));
        } else {
            self.map
                .insert(vote.validator_address().clone(), vec![(existing, vote)]);
        }
    }
}

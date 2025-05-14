//! Evidence of equivocation.

use alloc::collections::btree_map::BTreeMap;
use alloc::{vec, vec::Vec};

use derive_where::derive_where;

use malachitebft_core_types::{Context, SignedVote, Vote};

/// Keeps track of evidence of equivocation.
#[derive_where(Clone, Debug, Default)]
pub struct EvidenceMap<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::type_complexity)]
    map: BTreeMap<Ctx::Address, Vec<(SignedVote<Ctx>, SignedVote<Ctx>)>>,
    last: Option<(Ctx::Address, (SignedVote<Ctx>, SignedVote<Ctx>))>,
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

    /// Check if the given vote is the last equivocation recorded. If it is, return the
    /// address of the validator and the evidence.
    pub fn is_last_equivocation(
        &self,
        vote: &SignedVote<Ctx>,
    ) -> Option<(Ctx::Address, (SignedVote<Ctx>, SignedVote<Ctx>))> {
        self.last
            .as_ref()
            .filter(|(address, (_, conflicting))| {
                address == vote.validator_address() && conflicting == vote
            })
            .cloned()
    }

    /// Add evidence of equivocating votes, ie. two votes submitted by the same validator,
    /// but with different values but for the same height and round.
    ///
    /// # Precondition
    /// - Panics if the two conflicting votes were not proposed by the same validator.
    pub fn add(&mut self, existing: SignedVote<Ctx>, conflicting: SignedVote<Ctx>) {
        debug_assert_eq!(
            existing.validator_address(),
            conflicting.validator_address()
        );

        if let Some(evidence) = self.map.get_mut(conflicting.validator_address()) {
            evidence.push((existing.clone(), conflicting.clone()));
            self.last = Some((
                conflicting.validator_address().clone(),
                (existing, conflicting),
            ));
        } else {
            self.map.insert(
                conflicting.validator_address().clone(),
                vec![(existing.clone(), conflicting.clone())],
            );
            self.last = Some((
                conflicting.validator_address().clone(),
                (existing, conflicting),
            ));
        }
    }
}

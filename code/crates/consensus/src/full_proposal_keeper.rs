use std::collections::BTreeMap;

use derive_where::derive_where;
use tracing::{debug, warn};

use malachite_common::{Context, Proposal, Round, SignedProposal, Validity, Value};

use crate::ProposedValue;

/// This module is responsible for collecting proposed values and consensus proposal messages for
/// a given (height, round).
/// When a new_value is received from the value builder the following entry is stored:
/// `FullProposal { Some(new_value.value, new_value.validity), None }`
///
/// When a new_proposal is received from consensus gossip the following entry is stored:
/// `FullProposal { None, Some(new_proposal) }`
///
/// When both proposal and values have been received, the entry for (height, round) should be:
/// `FullProposal { Some(value.value, value.validity), Some(proposal) }`
///
/// It is possible that a proposer sends two (builder_value, proposal) pairs for same `(height, round)`.
/// In this case, both are stored and we consider that the proposer is equivocating.
/// Currently, the actual equivocation is caught deeper in the consensus crate, through consensus actor
/// propagating both proposals.
///
/// Note: In the future when we support implicit proposal message:
/// - store_proposal() will never be called
/// - get_full_proposal() should only check the presence of `builder_value`

#[derive_where(Clone, Debug)]
pub struct FullProposal<Ctx: Context> {
    // Value received from the builder and its validity.
    pub builder_value: Ctx::Value,
    pub validity: Validity,
    // Proposal consensus message
    pub proposal: SignedProposal<Ctx>,
}

impl<Ctx: Context> FullProposal<Ctx> {
    pub fn new(
        builder_value: Ctx::Value,
        validity: Validity,
        proposal: SignedProposal<Ctx>,
    ) -> Self {
        Self {
            builder_value,
            validity,
            proposal,
        }
    }
}

#[derive_where(Clone, Debug)]
enum Entry<Ctx: Context> {
    Full(FullProposal<Ctx>),
    ProposalOnly(SignedProposal<Ctx>),
    ValueOnly(Ctx::Value, Validity),
    // This is a placeholder for converting a partial
    // entry (`ProposalOnly` or `ValueOnly`) to a full entry (`Full`).
    // It is never actually stored in the keeper.
    Empty,
}

impl<Ctx: Context> Entry<Ctx> {
    fn full(value: Ctx::Value, validity: Validity, proposal: SignedProposal<Ctx>) -> Self {
        Entry::Full(FullProposal::new(value, validity, proposal))
    }
}

#[allow(clippy::derivable_impls)]
impl<Ctx: Context> Default for Entry<Ctx> {
    fn default() -> Self {
        Entry::Empty
    }
}

#[derive(Clone, Debug)]
pub struct FullProposalKeeper<Ctx: Context> {
    keeper: BTreeMap<(Ctx::Height, Round), Vec<Entry<Ctx>>>,
}

/// Replace a value in a mutable reference with a
/// new value if the old one matches the given pattern.
///
/// In our case, it temporarily replaces the entry with `Entry::Empty`,
/// and then replaces it with the new entry if the pattern matches.
macro_rules! replace_with {
    ($e:expr, $p:pat => $r:expr) => {
        *$e = match ::std::mem::take($e) {
            $p => $r,
            e => e,
        };
    };
}

impl<Ctx: Context> FullProposalKeeper<Ctx> {
    pub fn new() -> Self {
        Self {
            keeper: BTreeMap::new(),
        }
    }

    pub fn get_full_proposal(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Option<&FullProposal<Ctx>> {
        let entries = self
            .keeper
            .get(&(*height, round))
            .filter(|entries| !entries.is_empty())?;

        for entry in entries {
            match entry {
                Entry::Full(p) => {
                    if p.proposal.value().id() == value.id() {
                        return Some(p);
                    }
                }
                _ => continue,
            }
        }

        None
    }

    pub fn store_proposal(&mut self, new_proposal: SignedProposal<Ctx>) {
        let key = (new_proposal.height(), new_proposal.round());
        let entries = self.keeper.get_mut(&key);

        match entries {
            None => {
                // First time we see something (a proposal) for this height and round
                // Create a partial proposal with just the proposal
                self.keeper
                    .insert(key, vec![Entry::ProposalOnly(new_proposal)]);
            }
            Some(entries) => {
                // We have seen values and/ or proposals for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for entry in entries.iter_mut() {
                    match entry {
                        Entry::Full(full_proposal) => {
                            if full_proposal.proposal.value() == new_proposal.value() {
                                // Redundant proposal
                                return;
                            }
                        }
                        Entry::ValueOnly(value, _) => {
                            if value == new_proposal.value() {
                                // Found a matching value. Add the proposal
                                replace_with!(entry, Entry::ValueOnly(value, validity) => {
                                    Entry::full(value, validity, new_proposal)
                                });

                                return;
                            }
                        }
                        Entry::ProposalOnly(proposal) => {
                            if proposal.value() == new_proposal.value() {
                                // Redundant proposal
                                return;
                            }
                        }
                        Entry::Empty => {
                            // Should not happen
                            panic!("Empty entry found");
                        }
                    }
                }

                // Append new partial proposal
                entries.push(Entry::ProposalOnly(new_proposal));
            }
        }
    }

    pub fn store_value(&mut self, new_value: ProposedValue<Ctx>) {
        let key = (new_value.height, new_value.round);
        let entries = self.keeper.get_mut(&key);

        match entries {
            None => {
                // First time we see something (a proposed value) for this height and round
                // Create a full proposal with just the proposal
                let entry = Entry::ValueOnly(new_value.value, new_value.validity);
                self.keeper.insert(key, vec![entry]);
            }
            Some(entries) => {
                // We have seen proposals and/ or values for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for entry in entries.iter_mut() {
                    match entry {
                        Entry::ProposalOnly(proposal) => {
                            if proposal.value().id() == new_value.value.id() {
                                // Found a matching proposal. Change the entry at index i
                                replace_with!(entry, Entry::ProposalOnly(proposal) => {
                                    Entry::full(new_value.value, new_value.validity, proposal)
                                });

                                return;
                            }
                        }
                        Entry::ValueOnly(value, _) => {
                            if value.id() == new_value.value.id() {
                                // Same value received before, nothing to do.
                                return;
                            }
                        }
                        Entry::Full(full_proposal) => {
                            if full_proposal.proposal.value().id() == new_value.value.id() {
                                // Same value received before, nothing to do.
                                return;
                            }
                        }
                        Entry::Empty => {
                            // Should not happen
                            panic!("Empty entry found");
                        }
                    }
                }

                // Append new value
                entries.push(Entry::ValueOnly(new_value.value, new_value.validity));
            }
        }
    }

    pub fn remove_full_proposals(&mut self, height: Ctx::Height, round: Round) {
        // TODO - keep some heights back?
        debug!(%height, %round, "Removing full proposals");

        let result = self.keeper.remove_entry(&(height, round));
        match result {
            None => {
                warn!(%height, %round, "Full proposals absent");
            }
            Some((_key, removed)) => {
                debug!(%height, %round, "Removed {} full proposals", removed.len());
            }
        }
    }
}

impl<Ctx: Context> Default for FullProposalKeeper<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

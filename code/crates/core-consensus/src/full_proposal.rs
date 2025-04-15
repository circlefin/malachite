use std::collections::BTreeMap;

use derive_where::derive_where;

use malachitebft_core_types::{Context, Proposal, Round, SignedProposal, Validity, Value, ValueId};

use crate::ProposedValue;

/// A full proposal, ie. a proposal together with its value and validity.
#[derive_where(Clone, Debug)]
pub struct FullProposal<Ctx: Context> {
    /// Value received from the builder
    pub builder_value: Ctx::Value,
    /// Validity of the proposal
    pub validity: Validity,
    /// Proposal consensus message
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

/// An entry in the keeper.
#[derive_where(Clone, Debug)]
enum Entry<Ctx: Context> {
    /// The full proposal has been received,i.e. both the value and the proposal.
    Full(FullProposal<Ctx>),

    /// Only the proposal has been received.
    ProposalOnly(SignedProposal<Ctx>),

    /// Only the value has been received.
    ValueOnly(Ctx::Value, Validity),

    // This is a placeholder for converting a partial
    // entry (`ProposalOnly` or `ValueOnly`) to a full entry (`Full`).
    // It is never actually stored in the keeper.
    #[doc(hidden)]
    Empty,
}

impl<Ctx: Context> Entry<Ctx> {
    fn full(value: Ctx::Value, validity: Validity, proposal: SignedProposal<Ctx>) -> Self {
        Entry::Full(FullProposal::new(value, validity, proposal))
    }

    fn id(&self) -> Option<ValueId<Ctx>> {
        match self {
            Entry::Full(p) => Some(p.builder_value.id()),
            Entry::ProposalOnly(p) => Some(p.value().id()),
            Entry::ValueOnly(v, _) => Some(v.id()),
            Entry::Empty => None,
        }
    }
}

#[allow(clippy::derivable_impls)]
impl<Ctx: Context> Default for Entry<Ctx> {
    fn default() -> Self {
        Entry::Empty
    }
}

/// Keeper for collecting proposed values and consensus proposals for a given height and round.
///
/// When a new_value is received from the value builder the following entry is stored:
/// `Entry::ValueOnly(new_value.value, new_value.validity)`
///
/// When a new_proposal is received from consensus gossip the following entry is stored:
/// `Entry::ProposalOnly(new_proposal)`
///
/// When both proposal and values have been received, the entry for `(height, round)` should be:
/// `Entry::Full(FullProposal(value.value, value.validity, proposal))`
///
/// It is possible that a proposer sends two (builder_value, proposal) pairs for same `(height, round)`.
/// In this case both are stored, and we consider that the proposer is equivocating.
/// Currently, the actual equivocation is caught in the driver, through consensus actor
/// propagating both proposals.
///
/// When a new_proposal is received at most one complete proposal can be created. If a value at
/// proposal round is found, they are matched together. Otherwise, a value at the pol_round
/// is looked up and matched to form a full proposal (L28).
///
/// When a new value is received it is matched against the proposal at value round, and any proposal
/// at higher round with pol_round equal to the value round (L28). Therefore when a value is added
/// multiple complete proposals may form.
///
/// Note: For `parts_only` mode there is no explicit proposal wire message, instead
/// one is synthesized by the caller (`on_proposed_value` handler) before it invokes the `store_proposal` method.
#[derive_where(Clone, Debug, Default)]
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
        Self::default()
    }

    pub fn proposals_for_value(
        &self,
        proposed_value: &ProposedValue<Ctx>,
    ) -> Vec<SignedProposal<Ctx>> {
        let mut results = vec![];

        let first_key = &(proposed_value.height, proposed_value.round);
        let entries = self.keeper.range(first_key..);

        for (_, proposals) in entries {
            for entry in proposals {
                if let Entry::Full(p) = entry {
                    if p.proposal.value().id() == proposed_value.value.id() {
                        results.push(p.proposal.clone());
                    }
                }
            }
        }

        results
    }

    pub fn full_proposal_at_round_and_value(
        &self,
        height: &Ctx::Height,
        round: Round,
        value_id: &<Ctx::Value as Value>::Id,
    ) -> Option<&FullProposal<Ctx>> {
        let entries = self
            .keeper
            .get(&(*height, round))
            .filter(|entries| !entries.is_empty())?;

        for entry in entries {
            if let Entry::Full(p) = entry {
                if p.proposal.value().id() == *value_id {
                    return Some(p);
                }
            }
        }

        None
    }

    pub fn full_proposal_at_round_and_proposer(
        &self,
        height: &Ctx::Height,
        round: Round,
        proposer: &Ctx::Address,
    ) -> Option<&FullProposal<Ctx>> {
        let entries = self
            .keeper
            .get(&(*height, round))
            .filter(|entries| !entries.is_empty())?;

        for entry in entries {
            if let Entry::Full(p) = entry {
                if p.proposal.validator_address() == proposer {
                    return Some(p);
                }
            }
        }

        None
    }

    pub fn get_value<'a>(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &'a Ctx::Value,
    ) -> Option<(&'a Ctx::Value, Validity)> {
        let entries = self
            .keeper
            .get(&(*height, round))
            .filter(|entries| !entries.is_empty())?;

        for entry in entries {
            match entry {
                Entry::Full(p) if p.proposal.value().id() == value.id() => {
                    return Some((value, p.validity));
                }
                Entry::ValueOnly(v, validity) if v.id() == value.id() => {
                    return Some((value, *validity));
                }
                _ => continue,
            }
        }

        None
    }

    // Determines a new entry for L28 vs L22, L36, L49.
    // Called when a proposal is received, only if an entry for new_proposal's round and/ or value
    // is not found.
    fn new_entry(&self, new_proposal: SignedProposal<Ctx>) -> Entry<Ctx> {
        // L22, L36, L49
        if new_proposal.pol_round().is_nil() {
            return Entry::ProposalOnly(new_proposal);
        }

        // L28 - check if we have received a value at pol_round
        match self.get_value(
            &new_proposal.height(),
            new_proposal.pol_round(),
            new_proposal.value(),
        ) {
            // No value, create a proposal only entry
            None => Entry::ProposalOnly(new_proposal),

            // There is a value, create a full entry
            Some((v, validity)) => {
                Entry::Full(FullProposal::new(v.clone(), validity, new_proposal))
            }
        }
    }

    pub fn store_proposal(&mut self, new_proposal: SignedProposal<Ctx>) {
        let key = (new_proposal.height(), new_proposal.round());

        match self.keeper.get_mut(&key) {
            None => {
                // First time we see something (a proposal) for this height and round:
                // - if pol_round is Nil then create a partial proposal with just the proposal.
                // - if pol_round is defined and if a value at pol_round is present, add full entry,
                // - else just add the proposal.
                let new_entry = self.new_entry(new_proposal);
                self.keeper.insert(key, vec![new_entry]);
            }
            Some(entries) => {
                // We have seen values and/ or proposals for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for entry in entries.iter_mut() {
                    match entry {
                        Entry::Full(full_proposal) => {
                            if full_proposal.proposal.value().id() == new_proposal.value().id() {
                                // Redundant proposal, no need to check the pol_round if same value
                                return;
                            }
                        }
                        Entry::ValueOnly(value, _validity) => {
                            if value == new_proposal.value() {
                                // Found a matching value. Add the proposal
                                replace_with!(entry, Entry::ValueOnly(value, validity) => {
                                    Entry::full(value, validity, new_proposal)
                                });

                                return;
                            }
                        }
                        Entry::ProposalOnly(proposal) => {
                            if proposal.value().id() == new_proposal.value().id() {
                                // Redundant proposal, no need to check the pol_round if same value
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
                let new_entry = self.new_entry(new_proposal);
                self.keeper.entry(key).or_default().push(new_entry);
            }
        }
    }

    pub fn store_value(&mut self, new_value: &ProposedValue<Ctx>) {
        self.store_value_at_value_round(new_value);
        self.store_value_at_pol_round(new_value);
    }

    pub fn value_exists(&self, value: &ProposedValue<Ctx>) -> bool {
        match self.keeper.get(&(value.height, value.round)) {
            None => false,
            Some(entries) => entries
                .iter()
                .any(|entry| entry.id() == Some(value.value.id())),
        }
    }

    fn store_value_at_value_round(&mut self, new_value: &ProposedValue<Ctx>) {
        let key = (new_value.height, new_value.round);
        let entries = self.keeper.get_mut(&key);

        match entries {
            None => {
                // First time we see something (a proposed value) for this height and round
                // Create a full proposal with just the proposal
                let entry = Entry::ValueOnly(new_value.value.clone(), new_value.validity);
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
                                    Entry::full(new_value.value.clone(), new_value.validity, proposal)
                                });

                                return;
                            }
                        }
                        Entry::ValueOnly(value, ..) => {
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
                entries.push(Entry::ValueOnly(
                    new_value.value.clone(),
                    new_value.validity,
                ));
            }
        }
    }

    fn store_value_at_pol_round(&mut self, new_value: &ProposedValue<Ctx>) {
        let first_key = (new_value.height, new_value.round);

        // Get all entries for rounds higher than the value round, in case
        // there are proposals with pol_round equal to value round.
        let entries = self.keeper.range_mut(first_key..);

        for (_, proposals) in entries {
            // We may have seen proposals and/ or values for this height and round.
            // Iterate over the vector of full proposals and determine if a new entry needs
            // to be appended or an existing one has to be modified.
            for entry in proposals {
                if let Entry::ProposalOnly(proposal) = entry {
                    if proposal.value().id() == new_value.value.id()
                        && (proposal.round() == new_value.round
                            || proposal.pol_round() == new_value.round)
                    {
                        // Found a matching proposal. Change the entry at index i
                        replace_with!(entry, Entry::ProposalOnly(proposal) => {
                            Entry::full(new_value.value.clone(), new_value.validity, proposal)
                        });
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.keeper.clear();
    }
}

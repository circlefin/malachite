use std::collections::BTreeMap;
use tracing::{error, warn};

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
pub enum Entry<Ctx: Context> {
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

        for (_, proposals) in self.entries_at(proposed_value.height) {
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

    /// Look up a stored builder value by id at `height`, across all rounds (restream / mux).
    pub fn get_value_by_id(
        &self,
        height: &Ctx::Height,
        value_id: &ValueId<Ctx>,
    ) -> Option<(&Ctx::Value, Validity)> {
        for (_, entries) in self.entries_at(*height) {
            for entry in entries {
                match entry {
                    Entry::Full(p) if p.proposal.value().id() == *value_id => {
                        return Some((&p.builder_value, p.validity));
                    }
                    Entry::ValueOnly(v, validity) if v.id() == *value_id => {
                        return Some((v, *validity));
                    }
                    _ => {}
                }
            }
        }

        None
    }

    // Determines a new entry for L28 vs L22, L36, L49.
    // Called when a proposal is received, only if an entry for new_proposal's round and/ or value
    // is not found.
    fn new_entry(&self, new_proposal: SignedProposal<Ctx>) -> Entry<Ctx> {
        let value_id = new_proposal.value().id();
        if let Some((v, validity)) = self.get_value_by_id(&new_proposal.height(), &value_id) {
            return Entry::Full(FullProposal::new(v.clone(), validity, new_proposal));
        }

        Entry::ProposalOnly(new_proposal)
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
        self.upgrade_matching_proposals_at_height(new_value);
    }

    fn handle_validity_change(
        height: &Ctx::Height,
        round: Round,
        value_id: &ValueId<Ctx>,
        stored_validity: &mut Validity,
        new_validity: Validity,
        kind_phrase: &str,
    ) {
        use Validity::{Invalid, Valid};

        // Match previous behavior exactly:
        // - log warning and update for Invalid -> Valid
        // - log error but do not update for Valid -> Invalid
        match (*stored_validity, new_validity) {
            (Invalid, Valid) => {
                warn!(
                    height = %height,
                    round = %round,
                    value.id = ?value_id,
                    "Application changed its mind on {}'s validity: Invalid --> Valid",
                    kind_phrase
                );

                *stored_validity = new_validity;
            }
            (Valid, Invalid) => {
                error!(
                    height = %height,
                    round = %round,
                    value.id = ?value_id,
                    "Application changed its mind on {}'s validity: Valid --> Invalid; this should not happen",
                    kind_phrase
                );

                // Do not modify stored_validity per original behavior.
            }
            _ => {
                // No change in validity
            }
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
                        Entry::ValueOnly(old_value, old_validity) => {
                            if old_value.id() == new_value.value.id() {
                                // Same value received before; handle potential validity change.
                                Self::handle_validity_change(
                                    &new_value.height,
                                    new_value.round,
                                    &new_value.value.id(),
                                    old_validity,
                                    new_value.validity,
                                    "value",
                                );
                                return;
                            }
                        }
                        Entry::Full(full_proposal) => {
                            if full_proposal.proposal.value().id() == new_value.value.id() {
                                // Same value received before; handle potential validity change.
                                Self::handle_validity_change(
                                    &new_value.height,
                                    new_value.round,
                                    &new_value.value.id(),
                                    &mut full_proposal.validity,
                                    new_value.validity,
                                    "full proposal",
                                );
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

    /// Attach a stored payload to every outstanding `ProposalOnly` at this height that references
    /// the same value id (any `round` / `pol_round`), so restreamed parts can meet proposals.
    fn upgrade_matching_proposals_at_height(&mut self, new_value: &ProposedValue<Ctx>) {
        let Some((stored_value, stored_validity)) = self
            .get_value_by_id(&new_value.height, &new_value.value.id())
            .map(|(v, val)| (v.clone(), val))
        else {
            return;
        };

        for (_, proposals) in self.entries_at_mut(new_value.height) {
            for entry in proposals.iter_mut() {
                if let Entry::ProposalOnly(proposal) = entry {
                    if proposal.value().id() == new_value.value.id() {
                        replace_with!(entry, Entry::ProposalOnly(proposal) => {
                            Entry::full(stored_value.clone(), stored_validity, proposal)
                        });
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.keeper.clear();
    }

    /// Returns an iterator over all entries at a given height, across all rounds.
    fn entries_at(
        &self,
        height: Ctx::Height,
    ) -> impl Iterator<Item = (&(Ctx::Height, Round), &Vec<Entry<Ctx>>)> {
        self.keeper
            .range((height, Round::Nil)..)
            .take_while(move |((h, _), _)| h == &height)
    }

    /// Returns a mutable iterator over all entries at a given height, across all rounds.
    fn entries_at_mut(
        &mut self,
        height: Ctx::Height,
    ) -> impl Iterator<Item = (&(Ctx::Height, Round), &mut Vec<Entry<Ctx>>)> {
        self.keeper
            .range_mut((height, Round::Nil)..)
            .take_while(move |((h, _), _)| h == &height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use malachitebft_test::{Address, Height, TestContext, Value};

    fn addr() -> Address {
        Address::new([0; 20])
    }

    fn pv(height: u64, round: u32, value: u64) -> ProposedValue<TestContext> {
        ProposedValue {
            height: Height::new(height),
            round: Round::new(round),
            valid_round: Round::Nil,
            proposer: addr(),
            value: Value::new(value),
            validity: Validity::Valid,
        }
    }

    fn keys(keeper: &FullProposalKeeper<TestContext>, height: Height) -> Vec<(Height, Round)> {
        keeper.entries_at(height).map(|(k, _)| *k).collect()
    }

    fn keys_mut(
        keeper: &mut FullProposalKeeper<TestContext>,
        height: Height,
    ) -> Vec<(Height, Round)> {
        keeper.entries_at_mut(height).map(|(k, _)| *k).collect()
    }

    // --- entries_at ---

    #[test]
    fn entries_at_empty_keeper() {
        let keeper = FullProposalKeeper::<TestContext>::new();
        assert!(keeper.entries_at(Height::new(1)).next().is_none());
    }

    #[test]
    fn entries_at_nonexistent_height_returns_empty() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(3, 0, 30));

        assert!(keeper.entries_at(Height::new(2)).next().is_none());
    }

    #[test]
    fn entries_at_single_height_single_round() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));

        let height = Height::new(1);
        assert_eq!(keys(&keeper, height), vec![(height, Round::new(0))]);
    }

    #[test]
    fn entries_at_multiple_rounds_are_ordered_by_round() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        // Insert out of order to verify BTreeMap ordering.
        keeper.store_value(&pv(1, 2, 12));
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 1, 11));

        let height = Height::new(1);
        assert_eq!(
            keys(&keeper, height),
            vec![
                (height, Round::new(0)),
                (height, Round::new(1)),
                (height, Round::new(2)),
            ]
        );
    }

    #[test]
    fn entries_at_skips_lower_heights() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 5, 15));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(2, 1, 21));

        let height = Height::new(2);
        assert_eq!(
            keys(&keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(1))]
        );
    }

    #[test]
    fn entries_at_stops_before_higher_heights() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 1, 11));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(3, 0, 30));

        let height = Height::new(1);
        assert_eq!(
            keys(&keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(1))]
        );
    }

    #[test]
    fn entries_at_isolates_target_height_between_others() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(2, 3, 23));
        keeper.store_value(&pv(3, 0, 30));
        keeper.store_value(&pv(4, 0, 40));

        let height = Height::new(2);
        assert_eq!(
            keys(&keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(3))]
        );
    }

    #[test]
    fn entries_at_exposes_stored_entries() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 0, 20)); // second value at same round

        let entries: Vec<_> = keeper.entries_at(Height::new(1)).collect();
        assert_eq!(entries.len(), 1);

        let (_, bucket) = entries[0];
        assert_eq!(bucket.len(), 2);

        let value_ids: Vec<_> = bucket
            .iter()
            .map(|e| match e {
                Entry::ValueOnly(v, _) => v.id(),
                other => panic!("expected ValueOnly entry, got {other:?}"),
            })
            .collect();
        assert_eq!(value_ids, vec![Value::new(10).id(), Value::new(20).id()]);
    }

    // --- entries_at_mut ---

    #[test]
    fn entries_at_mut_empty_keeper() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        assert!(keeper.entries_at_mut(Height::new(1)).next().is_none());
    }

    #[test]
    fn entries_at_mut_nonexistent_height_returns_empty() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(3, 0, 30));

        assert!(keeper.entries_at_mut(Height::new(2)).next().is_none());
    }

    #[test]
    fn entries_at_mut_single_height_single_round() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));

        let height = Height::new(1);
        assert_eq!(keys_mut(&mut keeper, height), vec![(height, Round::new(0))]);
    }

    #[test]
    fn entries_at_mut_multiple_rounds_are_ordered_by_round() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 2, 12));
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 1, 11));

        let height = Height::new(1);
        assert_eq!(
            keys_mut(&mut keeper, height),
            vec![
                (height, Round::new(0)),
                (height, Round::new(1)),
                (height, Round::new(2)),
            ]
        );
    }

    #[test]
    fn entries_at_mut_skips_lower_heights() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 5, 15));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(2, 1, 21));

        let height = Height::new(2);
        assert_eq!(
            keys_mut(&mut keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(1))]
        );
    }

    #[test]
    fn entries_at_mut_stops_before_higher_heights() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 1, 11));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(3, 0, 30));

        let height = Height::new(1);
        assert_eq!(
            keys_mut(&mut keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(1))]
        );
    }

    #[test]
    fn entries_at_mut_isolates_target_height_between_others() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(2, 0, 20));
        keeper.store_value(&pv(2, 3, 23));
        keeper.store_value(&pv(3, 0, 30));
        keeper.store_value(&pv(4, 0, 40));

        let height = Height::new(2);
        assert_eq!(
            keys_mut(&mut keeper, height),
            vec![(height, Round::new(0)), (height, Round::new(3))]
        );
    }

    #[test]
    fn entries_at_mut_allows_in_place_mutation() {
        let mut keeper = FullProposalKeeper::<TestContext>::new();
        keeper.store_value(&pv(1, 0, 10));
        keeper.store_value(&pv(1, 1, 11));
        // Noise at other heights to ensure we don't touch them.
        keeper.store_value(&pv(2, 0, 20));

        // Mutate every bucket at height 1: replace the stored value's validity with Invalid.
        for (_, bucket) in keeper.entries_at_mut(Height::new(1)) {
            for entry in bucket.iter_mut() {
                if let Entry::ValueOnly(_, validity) = entry {
                    *validity = Validity::Invalid;
                }
            }
        }

        // All entries at height 1 are now Invalid.
        for (_, bucket) in keeper.entries_at(Height::new(1)) {
            for entry in bucket {
                match entry {
                    Entry::ValueOnly(_, validity) => assert_eq!(*validity, Validity::Invalid),
                    other => panic!("expected ValueOnly entry, got {other:?}"),
                }
            }
        }

        // Entries at other heights are unchanged.
        for (_, bucket) in keeper.entries_at(Height::new(2)) {
            for entry in bucket {
                match entry {
                    Entry::ValueOnly(_, validity) => assert_eq!(*validity, Validity::Valid),
                    other => panic!("expected ValueOnly entry, got {other:?}"),
                }
            }
        }
    }
}

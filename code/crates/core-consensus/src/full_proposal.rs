use std::collections::BTreeMap;
use tracing::{error, warn};

use derive_where::derive_where;

use malachitebft_core_types::{Context, Proposal, Round, SignedProposal, Validity, Value, ValueId};

use crate::ProposedValue;

/// Maximum number of distinct entries stored per `(height, round)`.
///
/// Two entries are sufficient to detect and record equivocation.
pub const MAX_PROPOSALS_PER_ROUND: usize = 2;

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

    /// The value id this entry references, if any.
    fn value_id(&self) -> Option<ValueId<Ctx>> {
        match self {
            Entry::Full(p) => Some(p.proposal.value().id()),
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

/// Outcome of [`FullProposalKeeper::store_proposal`].
#[must_use]
#[derive_where(Clone, Debug)]
pub enum StoreProposalResult<Ctx: Context> {
    /// The proposal was stored as a new entry, or it upgraded an existing entry to `Full`.
    Stored,
    /// The proposal was an exact duplicate of one already stored, and was ignored.
    DuplicateIgnored,
    /// The proposal was rejected because the per-`(height, round)` cap was already reached.
    CapReached,
    /// A different proposal with the same value id is already present for this `(height, round)`.
    /// The two proposals differ in at least one field (e.g. `pol_round`), so the same proposer
    /// has equivocated. Both proposals are returned so the caller can record evidence.
    Equivocation {
        existing: SignedProposal<Ctx>,
        conflicting: SignedProposal<Ctx>,
    },
}

/// Keeper for collecting proposed values and consensus proposals for a given height and round.
///
/// Each `(height, round)` holds a small vector of [`Entry`] values, where an entry records how much
/// of a proposed value is currently held:
///
/// - `Entry::ValueOnly(value, validity)` — a value arrived from the value builder, with no matching
///   proposal yet.
/// - `Entry::ProposalOnly(proposal)` — a proposal arrived over consensus gossip, with no matching
///   value yet.
/// - `Entry::Full(value, validity, proposal)` — both halves are present. It is formed when the
///   second half arrives: a proposal that finds an existing value, or a value that finds an
///   existing proposal.
/// - `Entry::Empty` — never stored; a transient placeholder used by `replace_with!` while an entry
///   is upgraded in place.
///
/// A proposal and a value are paired **by value id alone**, searched across every round at the
/// height (see `get_value_by_id`). `round` and `pol_round` are app-side metadata and take no part
/// in pairing. So a new proposal becomes `Full` if a value with the same id is already stored at
/// the height and `ProposalOnly` otherwise; symmetrically for a new value. Because matching spans
/// rounds, a single incoming value can complete several `ProposalOnly` entries at once (see
/// `upgrade_matching_proposals_at_height`); validity is reconciled along the way (`Invalid -> Valid`
/// propagates, `Valid -> Invalid` is logged and rejected).
///
/// A proposer may send more than one proposal for the same `(height, round)`:
/// - Distinct value ids are stored as separate entries; the driver flags the equivocation as each
///   entry is forwarded to it.
/// - The same value id differing in any other field (e.g. `pol_round`) keeps only the first entry
///   and reports the conflict via [`StoreProposalResult::Equivocation`] so evidence is still
///   recorded; an exact duplicate is ignored.
///
/// At most [`MAX_PROPOSALS_PER_ROUND`] distinct entries are retained per `(height, round)`,
/// beyond which an entry is admitted only when the caller declares it exempt.
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

    /// Returns `true` if storing an entry with `value_id` at `(height, round)` would append a new
    /// distinct entry beyond [`MAX_PROPOSALS_PER_ROUND`]. A value id already present at the key
    /// upgrades or matches an existing entry and never grows the bucket, so it is not rejected.
    ///
    /// This reports bucket growth alone. Admission is decided by `State::exceeds_per_round_cap`,
    /// which also admits an entry backed by a polka certificate, so `true` here does not on its
    /// own mean the message is dropped.
    pub fn would_append_distinct(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> bool {
        let Some(entries) = self.keeper.get(&(height, round)) else {
            return false;
        };

        entries.len() >= MAX_PROPOSALS_PER_ROUND
            && !entries
                .iter()
                .any(|e| e.value_id().as_ref() == Some(value_id))
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

    // Build the entry for a proposal that has no matching entry yet: `Full` if a value with the
    // same id is already stored at this height (any round), otherwise `ProposalOnly`.
    fn new_entry(&self, new_proposal: SignedProposal<Ctx>) -> Entry<Ctx> {
        let value_id = new_proposal.value().id();
        if let Some((v, validity)) = self.get_value_by_id(&new_proposal.height(), &value_id) {
            return Entry::Full(FullProposal::new(v.clone(), validity, new_proposal));
        }

        Entry::ProposalOnly(new_proposal)
    }

    /// Store a proposal, pairing it with a matching value when one is already present.
    ///
    /// `cap_exempt` admits the proposal even when the `(height, round)` bucket is full. The
    /// caller owns that decision, as the keeper holds no certificates of its own.
    pub fn store_proposal(
        &mut self,
        new_proposal: SignedProposal<Ctx>,
        cap_exempt: bool,
    ) -> StoreProposalResult<Ctx> {
        let key = (new_proposal.height(), new_proposal.round());

        match self.keeper.get_mut(&key) {
            None => {
                // First entry at this `(height, round)`: `Full` if a value with the same id is
                // already stored at this height, otherwise `ProposalOnly`.
                let new_entry = self.new_entry(new_proposal);
                self.keeper.insert(key, vec![new_entry]);
                StoreProposalResult::Stored
            }
            Some(entries) => {
                // We have seen values and/ or proposals for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for entry in entries.iter_mut() {
                    match entry {
                        Entry::Full(full_proposal) => {
                            if full_proposal.proposal.value().id() == new_proposal.value().id() {
                                return if full_proposal.proposal == new_proposal {
                                    // Exact duplicate (same signature): silently ignore.
                                    StoreProposalResult::DuplicateIgnored
                                } else {
                                    // Same value id but a different proposal: the proposer has
                                    // equivocated. One entry per `(height, round, value_id)` is
                                    // kept, so surface the equivocation to the caller instead of
                                    // pushing.
                                    StoreProposalResult::Equivocation {
                                        existing: full_proposal.proposal.clone(),
                                        conflicting: new_proposal,
                                    }
                                };
                            }
                        }
                        Entry::ValueOnly(value, _validity) => {
                            if value == new_proposal.value() {
                                // Found a matching value. Add the proposal
                                replace_with!(entry, Entry::ValueOnly(value, validity) => {
                                    Entry::full(value, validity, new_proposal)
                                });

                                return StoreProposalResult::Stored;
                            }
                        }
                        Entry::ProposalOnly(proposal) => {
                            if proposal.value().id() == new_proposal.value().id() {
                                return if *proposal == new_proposal {
                                    StoreProposalResult::DuplicateIgnored
                                } else {
                                    // Same value id but a different proposal: the proposer has
                                    // equivocated.
                                    StoreProposalResult::Equivocation {
                                        existing: proposal.clone(),
                                        conflicting: new_proposal,
                                    }
                                };
                            }
                        }
                        Entry::Empty => {
                            // Should not happen
                            panic!("Empty entry found");
                        }
                    }
                }

                // Append new partial proposal, unless the per-(height, round) cap is reached.
                if !cap_exempt && entries.len() >= MAX_PROPOSALS_PER_ROUND {
                    warn!(
                        height = %key.0,
                        round = %key.1,
                        cap = MAX_PROPOSALS_PER_ROUND,
                        "Rejecting additional distinct proposal: per-(height, round) cap reached"
                    );
                    return StoreProposalResult::CapReached;
                }

                let new_entry = self.new_entry(new_proposal);
                self.keeper.entry(key).or_default().push(new_entry);
                StoreProposalResult::Stored
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
                // First entry at this `(height, round)`: store the value on its own as
                // `ValueOnly`.
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

                // Append new value. This path is intentionally NOT capped at the keeper layer:
                // callers must pre-gate non-sync values via `exceeds_per_round_cap`, while sync
                // values (carrying verified commit certificates) are allowed to bypass the cap.
                entries.push(Entry::ValueOnly(
                    new_value.value.clone(),
                    new_value.validity,
                ));
            }
        }
    }

    /// Apply `new_value`'s `(value, validity)` to every entry at the same height that
    /// references the same value id (matching by value id only — `round` / `pol_round`
    /// are app-side metadata; validity is a property of `(height, value_id)`).
    ///
    /// - `ProposalOnly` → upgrade to `Full` so restreamed parts can meet proposals.
    /// - `Full` → reconcile validity via `handle_validity_change` (only `Invalid -> Valid`
    ///   propagates; `Valid -> Invalid` is logged and rejected).
    /// - `ValueOnly` → reconcile validity the same way.
    fn upgrade_matching_proposals_at_height(&mut self, new_value: &ProposedValue<Ctx>) {
        for ((_, round), proposals) in self.entries_at_mut(new_value.height) {
            for entry in proposals.iter_mut() {
                match entry {
                    Entry::ProposalOnly(proposal)
                        if proposal.value().id() == new_value.value.id() =>
                    {
                        replace_with!(entry, Entry::ProposalOnly(proposal) => {
                            Entry::full(new_value.value.clone(), new_value.validity, proposal)
                        });
                    }
                    Entry::Full(full_proposal)
                        if full_proposal.proposal.value().id() == new_value.value.id() =>
                    {
                        Self::handle_validity_change(
                            &new_value.height,
                            full_proposal.proposal.round(),
                            &new_value.value.id(),
                            &mut full_proposal.validity,
                            new_value.validity,
                            "full proposal at other round",
                        );
                    }
                    Entry::ValueOnly(value, validity) if value.id() == new_value.value.id() => {
                        Self::handle_validity_change(
                            &new_value.height,
                            *round,
                            &new_value.value.id(),
                            validity,
                            new_value.validity,
                            "value at other round",
                        );
                    }
                    _ => {}
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

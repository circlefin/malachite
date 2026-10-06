use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::info;

use malachitebft_core_driver::Driver;

use crate::full_proposal::{FullProposal, FullProposalKeeper, StoreProposalResult};
use crate::input::Input;
use crate::params::Params;
use crate::prelude::*;
use crate::types::ProposedValue;
use crate::util::bounded_queue::BoundedQueue;
use crate::wal_replay_index::ReplayIndex;

/// Where an admitted proposed value is retained in the full-proposal keeper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProposedValueStorage {
    /// Retain the value at its source round and pair it with matching proposals at this height.
    SourceRoundAndMatchingEntries,
    /// Retain the value at a round where it has a polka certificate. Only the certified value can
    /// grow that round's bucket through this path, and at most one value can hold a polka per round,
    /// so the bucket retains at most `MAX_PROPOSALS_PER_ROUND + 1` entries.
    PolkaRoundOnly(Round),
    /// Update matching entries across the height without growing the source-round bucket.
    MatchingEntriesOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProposedValueStorageError {
    /// The source-round bucket is full, and no matching entry or polka certificate is available.
    NoBoundedPlacement,
}

impl fmt::Display for ProposedValueStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBoundedPlacement => f.write_str(
                "the source-round bucket is full, and no matching entry or polka certificate is available",
            ),
        }
    }
}

/// Whether a stored proposal must be persisted in the write-ahead log.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalPersistence {
    /// The proposal contributed state that must survive a restart.
    Required,
    /// The proposal did not contribute retained state.
    NotRequired,
}

/// The state maintained by consensus for processing a [`Input`].
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// The context for the consensus state machine
    pub ctx: Ctx,

    /// The consensus parameters
    pub params: Params<Ctx>,

    /// Driver for the per-round consensus state machine
    pub driver: Driver<Ctx>,

    /// A queue of inputs that were received before the driver started.
    pub input_queue: BoundedQueue<Ctx::Height, Input<Ctx>>,

    /// The proposals to decide on.
    pub full_proposal_keeper: FullProposalKeeper<Ctx>,

    /// Last prevote broadcasted by this node
    pub last_signed_prevote: Option<SignedVote<Ctx>>,

    /// Last precommit broadcasted by this node
    pub last_signed_precommit: Option<SignedVote<Ctx>>,

    /// Target time for the current height
    pub target_time: Option<Duration>,

    /// Vote-extension policy for the current height.
    pub vote_extension_policy: VoteExtensionPolicy,

    /// Start time of the current height
    pub height_start_time: Option<Instant>,

    /// Index of the messages in the write-ahead log for this height. Empty once replay ends.
    pub replay_index: ReplayIndex<Ctx>,

    /// Whether the inputs currently being fed to consensus come from the write-ahead log
    /// rather than from the network. Tracks the same window as `replay_index`, which is
    /// also empty for a log holding none of our own messages.
    ///
    /// Admission bounds that exist to cap the work an untrusted peer can induce do not
    /// apply to replayed inputs: every one of them was already admitted, verified and
    /// applied by an earlier run of this node, so re-reading them cannot grow state
    /// beyond what that run already held. Applying such a bound to replay instead makes
    /// the read path reject what the write path accepted, losing state the log records.
    replaying_wal: bool,

    /// Whether we are in the finalization period.
    ///
    /// The finalization period is entered in decide, cleared in finalize_height,
    /// and only valid during the commit step.
    ///
    /// It allows collecting additional precommits for the decided value after
    /// the decision is made in decide, which can be included in the commit certificate.
    pub finalization_period: bool,

    /// This node's own non-nil precommit extensions recovered from the WAL.
    ///
    /// WAL replay feeds stored entries back through the ordinary input path, so
    /// the driver may emit a local precommit again. `extend_vote` uses this map
    /// instead of asking the application for a fresh extension on a precommit
    /// that was already signed, logged, and published.
    recovered_vote_extensions: BTreeMap<(Round, ValueId<Ctx>), SignedExtension<Ctx>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        ctx: Ctx,
        height: Ctx::Height,
        validator_set: Ctx::ValidatorSet,
        params: Params<Ctx>,
        queue_capacity: usize,
        queue_per_height_capacity: usize,
    ) -> Self {
        let driver = Driver::new(
            ctx.clone(),
            height,
            validator_set,
            params.address.clone(),
            params.threshold_params,
        );

        Self {
            ctx,
            driver,
            params,
            input_queue: BoundedQueue::new(queue_capacity, queue_per_height_capacity),
            full_proposal_keeper: Default::default(),
            last_signed_prevote: None,
            last_signed_precommit: None,
            target_time: None,
            vote_extension_policy: VoteExtensionPolicy::default(),
            height_start_time: None,
            replay_index: ReplayIndex::default(),
            replaying_wal: false,
            finalization_period: false,
            recovered_vote_extensions: BTreeMap::new(),
        }
    }

    /// Index the messages we recorded in the write-ahead log, ahead of replaying it.
    ///
    /// Also marks the replay as under way, so that [`Self::is_replaying_wal`] holds for
    /// every entry fed in until [`Self::reset_entries_index`] ends it.
    pub fn index_wal_entries<'a, I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = &'a Input<Ctx>>,
        Ctx: 'a,
    {
        let index = ReplayIndex::from_wal_entries(entries, self.address());
        self.replay_index = index;
        self.replaying_wal = true;
    }

    /// Drop the replay index once the write-ahead log has been replayed.
    pub fn reset_entries_index(&mut self) {
        self.replay_index = ReplayIndex::default();
        self.replaying_wal = false;
    }

    pub fn height(&self) -> Ctx::Height {
        self.driver.height()
    }

    pub fn round(&self) -> Round {
        self.driver.round()
    }

    /// Whether consensus is currently replaying inputs from the Write-Ahead Log.
    pub fn is_replaying_wal(&self) -> bool {
        self.replaying_wal
    }

    pub fn address(&self) -> &Ctx::Address {
        self.driver.address()
    }

    pub fn validator_set(&self) -> &Ctx::ValidatorSet {
        self.driver.validator_set()
    }

    pub fn get_proposer(&self, height: Ctx::Height, round: Round) -> &Ctx::Address {
        self.ctx
            .select_proposer(self.validator_set(), height, round)
            .address()
    }

    pub fn set_last_vote(&mut self, vote: SignedVote<Ctx>) {
        match vote.vote_type() {
            VoteType::Prevote => self.last_signed_prevote = Some(vote),
            VoteType::Precommit => self.last_signed_precommit = Some(vote),
        }
    }

    /// Record this node's own non-nil precommit extensions from previously
    /// logged inputs so WAL replay can reuse them instead of asking the
    /// application for a fresh extension.
    ///
    /// The first logged extension for each `(round, value_id)` is kept: that
    /// is the one peers already saw.
    pub fn record_recovered_own_precommit_extensions<'a>(
        &mut self,
        inputs: impl IntoIterator<Item = &'a Input<Ctx>>,
    ) {
        let address = self.address().clone();
        let height = self.height();

        for input in inputs {
            let Some((round, value_id, extension)) =
                own_non_nil_precommit_extension(input, &address, &height)
            else {
                continue;
            };

            self.recovered_vote_extensions
                .entry((round, value_id))
                .or_insert_with(|| extension.clone());
        }
    }

    /// The extension this node already signed for a non-nil precommit at
    /// `(round, value_id)`, if one was recovered from the WAL.
    pub fn recovered_vote_extension(
        &self,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> Option<&SignedExtension<Ctx>> {
        self.recovered_vote_extensions
            .get(&(round, value_id.clone()))
    }

    pub fn restore_precommits(
        &self,
        height: Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Vec<SignedVote<Ctx>> {
        assert_eq!(height, self.driver.height());
        self.driver.restore_precommits(round, &value.id())
    }

    /// Get the polka certificate at the current height for the specified round and value, if it exists
    pub fn polka_certificate(
        &self,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> Option<&PolkaCertificate<Ctx>> {
        self.driver.polka_certificate(round, value_id)
    }

    /// Whether recording `vote` would yield a new equivocation pair.
    ///
    /// Answering this needs no signature verification, so it can gate the cost of verifying
    /// `vote` on there being something to prove.
    pub fn can_record_equivocation(&self, vote: &SignedVote<Ctx>) -> bool {
        self.driver.votes().can_record_equivocation(vote)
    }

    /// Record `vote` as equivocation evidence against the vote already held for the same
    /// round, type and validator, returning the pair only when it is newly recorded.
    pub fn record_vote_evidence(&mut self, vote: SignedVote<Ctx>) -> Option<DoubleVote<Ctx>> {
        self.driver.votes_mut().detect_equivocation(vote)
    }

    fn polka_certificate_round_for_value(
        &self,
        height: Ctx::Height,
        preferred_round: Round,
        value_id: &ValueId<Ctx>,
    ) -> Option<Round> {
        if self
            .polka_certificate(preferred_round, value_id)
            .is_some_and(|certificate| certificate.height == height)
        {
            return Some(preferred_round);
        }

        self.driver
            .votes()
            .all_rounds()
            .keys()
            .copied()
            .find(|round| {
                self.polka_certificate(*round, value_id)
                    .is_some_and(|certificate| certificate.height == height)
            })
    }

    pub fn full_proposal_at_round_and_value(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Option<&FullProposal<Ctx>> {
        self.full_proposal_keeper
            .full_proposal_at_round_and_value(height, round, &value.id())
    }

    pub fn full_proposal_at_round_and_proposer(
        &self,
        height: &Ctx::Height,
        round: Round,
        address: &Ctx::Address,
    ) -> Option<&FullProposal<Ctx>> {
        self.full_proposal_keeper
            .full_proposal_at_round_and_proposer(height, round, address)
    }

    /// Get a proposed value by its ID at the specified height.
    /// `round` simply populates the corresponding field on the
    /// returned `ProposedValue`.
    pub fn get_proposed_value_by_id(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> Option<ProposedValue<Ctx>> {
        let (value, validity) = self
            .full_proposal_keeper
            .get_value_by_id(&height, value_id)?;
        Some(ProposedValue {
            height,
            round,
            valid_round: Round::Nil,
            proposer: self.get_proposer(height, round).clone(),
            value: value.clone(),
            validity,
        })
    }

    pub fn proposals_for_value(
        &self,
        proposed_value: &ProposedValue<Ctx>,
    ) -> Vec<SignedProposal<Ctx>> {
        self.full_proposal_keeper
            .proposals_for_value(proposed_value)
    }

    /// Returns `true` if storing an entry with `value_id` at `(height, round)` would append a new
    /// distinct entry beyond the per-`(height, round)` cap. Used to drop a message before it
    /// reaches the WAL.
    ///
    /// An entry whose value already holds a polka certificate at `round` is admitted regardless
    /// of the cap: the certificate carries a quorum of signed prevotes, so at most one value per
    /// round can qualify.
    pub fn exceeds_per_round_cap(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> bool {
        self.full_proposal_keeper
            .would_append_distinct(height, round, value_id)
            && self.polka_certificate(round, value_id).is_none()
    }

    /// Select where to retain a proposed value, or reject it if no bounded placement exists.
    pub(crate) fn proposed_value_storage(
        &self,
        height: Ctx::Height,
        round: Round,
        value_id: &ValueId<Ctx>,
    ) -> Result<ProposedValueStorage, ProposedValueStorageError> {
        if !self
            .full_proposal_keeper
            .would_append_distinct(height, round, value_id)
        {
            return Ok(ProposedValueStorage::SourceRoundAndMatchingEntries);
        }

        if self
            .full_proposal_keeper
            .has_matching_entry(height, value_id)
        {
            return Ok(ProposedValueStorage::MatchingEntriesOnly);
        }

        self.polka_certificate_round_for_value(height, round, value_id)
            .map(ProposedValueStorage::PolkaRoundOnly)
            .ok_or(ProposedValueStorageError::NoBoundedPlacement)
    }

    /// Store a proposal and report whether it must be persisted in the write-ahead log.
    pub fn store_proposal(
        &mut self,
        new_proposal: SignedProposal<Ctx>,
        _metrics: &Metrics,
    ) -> ProposalPersistence {
        let cap_exempt = self
            .polka_certificate(new_proposal.round(), &new_proposal.value().id())
            .is_some();

        match self
            .full_proposal_keeper
            .store_proposal(new_proposal, cap_exempt)
        {
            StoreProposalResult::Stored => ProposalPersistence::Required,
            StoreProposalResult::DuplicateIgnored => ProposalPersistence::NotRequired,
            StoreProposalResult::CapReached => {
                // Backstop: the proposal handler's `exceeds_per_round_cap` pre-gate already drops
                // and counts over-cap proposals before calling here, so this only fires if
                // `store_proposal` is ever called from a path without that pre-gate.
                #[cfg(feature = "metrics")]
                _metrics.dropped_capped_proposals.inc();

                ProposalPersistence::NotRequired
            }
            StoreProposalResult::Equivocation {
                existing,
                conflicting,
            } => {
                // The keeper filters same-value-id proposals to preserve its at-most-one-entry
                // invariant. Surface the equivocation to the driver's evidence map so it is not
                // lost when the conflicting proposal differs only in `pol_round`.
                if self.driver.record_proposal_evidence(existing, conflicting) {
                    ProposalPersistence::Required
                } else {
                    ProposalPersistence::NotRequired
                }
            }
            StoreProposalResult::StoredWithEquivocation {
                existing,
                conflicting,
            } => {
                // Proposals with distinct value ids are all retained, so the proposal itself is
                // state that must survive a restart whether or not the evidence map had room for
                // any pair. Every pair is offered; the evidence map applies its own dedup and
                // per-validator cap.
                for existing in existing {
                    let _ = self
                        .driver
                        .record_proposal_evidence(existing, conflicting.clone());
                }

                ProposalPersistence::Required
            }
        }
    }

    /// Store a proposed value at its source round and return its reconciled validity.
    /// Consensus inputs use the crate-private policy-aware method after admission validation.
    pub fn store_value(&mut self, new_value: &ProposedValue<Ctx>) -> Validity {
        self.store_value_with_storage(
            new_value,
            ProposedValueStorage::SourceRoundAndMatchingEntries,
        )
    }

    /// Store an admitted proposed value according to its validated storage policy.
    pub(crate) fn store_value_with_storage(
        &mut self,
        new_value: &ProposedValue<Ctx>,
        storage: ProposedValueStorage,
    ) -> Validity {
        // Values for higher height should have been cached for future processing
        assert_eq!(new_value.height, self.driver.height());

        if self
            .full_proposal_keeper
            .get_value_by_id(&new_value.height, &new_value.value.id())
            .is_none()
            && new_value.validity.is_invalid()
        {
            warn!(
                height = %new_value.height,
                round = %new_value.round,
                value.id = ?new_value.value.id(),
                "Application sent an invalid proposed value"
            );
        }
        match storage {
            ProposedValueStorage::MatchingEntriesOnly => self
                .full_proposal_keeper
                .store_value_in_matching_entries(new_value),
            ProposedValueStorage::SourceRoundAndMatchingEntries => {
                self.full_proposal_keeper.store_value(new_value)
            }
            ProposedValueStorage::PolkaRoundOnly(round) => self
                .full_proposal_keeper
                .store_value_at_round_only(new_value, round),
        }

        // Retrieve the validity after storing, as it may have changed (e.g., from Invalid to Valid)
        let (_value, validity) = self
            .full_proposal_keeper
            .get_value_by_id(&new_value.height, &new_value.value.id())
            .expect("The selected placement stored the value or updated a matching entry");

        validity
    }

    pub fn reset_and_start_height(
        &mut self,
        height: Ctx::Height,
        validator_set: Ctx::ValidatorSet,
        target_time: Option<Duration>,
        vote_extension_policy: VoteExtensionPolicy,
    ) {
        let previous_height = self.height();
        let previous_vote_extension_policy = self.vote_extension_policy;
        if previous_vote_extension_policy.is_required() && vote_extension_policy.is_disabled() {
            warn!(
                previous.height = %previous_height,
                new.height = %height,
                previous.policy = ?previous_vote_extension_policy,
                new.policy = ?vote_extension_policy,
                "Vote extension policy moved from required-present to required-absent"
            );
        }

        self.full_proposal_keeper.clear();
        self.last_signed_prevote = None;
        self.last_signed_precommit = None;
        self.target_time = target_time;
        self.vote_extension_policy = vote_extension_policy;
        self.height_start_time = Some(Instant::now());
        self.finalization_period = false;
        self.recovered_vote_extensions.clear();

        self.driver.move_to_height(height, validator_set);
    }

    /// Return the round and value id of the decided value.
    pub fn decided_value(&self) -> Option<(Round, Ctx::Value)> {
        self.driver.decided_value()
    }

    /// Queue an input for later processing, only keep inputs for the highest height seen so far.
    pub fn buffer_input(&mut self, height: Ctx::Height, input: Input<Ctx>, _metrics: &Metrics) {
        self.input_queue.push(height, input);

        #[cfg(feature = "metrics")]
        {
            _metrics.queue_heights.set(self.input_queue.len() as i64);
            _metrics.queue_size.set(self.input_queue.size() as i64);
        }
    }

    /// Take all inputs that are pending for the specified height and remove from the input queue.
    pub fn take_pending_inputs(&mut self, _metrics: &Metrics) -> Vec<Input<Ctx>>
    where
        Ctx: Context,
    {
        let inputs = self
            .input_queue
            .shift_and_take(&self.height())
            .collect::<Vec<_>>();

        #[cfg(feature = "metrics")]
        {
            _metrics.queue_heights.set(self.input_queue.len() as i64);
            _metrics.queue_size.set(self.input_queue.size() as i64);
        }

        inputs
    }

    pub fn print_state(&self) {
        if let Some(per_round) = self.driver.votes().per_round(self.driver.round()) {
            info!(
                "Number of validators having voted: {} / {}",
                per_round.addresses_weights().get_inner().len(),
                self.driver.validator_set().count()
            );
            info!(
                "Total voting power of validators: {}",
                self.driver.validator_set().total_voting_power()
            );
            info!(
                "Voting power required: {}",
                self.params
                    .threshold_params
                    .quorum
                    .min_expected(self.driver.validator_set().total_voting_power())
            );
            info!(
                "Total voting power of validators having voted: {}",
                per_round.addresses_weights().sum()
            );
            info!(
                "Total voting power of validators having prevoted nil: {}",
                per_round
                    .votes()
                    .get_weight(VoteType::Prevote, &NilOrVal::Nil)
            );
            info!(
                "Total voting power of validators having precommited nil: {}",
                per_round
                    .votes()
                    .get_weight(VoteType::Precommit, &NilOrVal::Nil)
            );
            info!(
                "Total weight of prevotes: {}",
                per_round.votes().weight_sum(VoteType::Prevote)
            );
            info!(
                "Total weight of precommits: {}",
                per_round.votes().weight_sum(VoteType::Precommit)
            );
        }
    }

    /// Check if this node is an active validator.
    ///
    /// Returns true only if:
    /// - Consensus is enabled in the configuration, AND
    /// - This node is present in the current validator set
    pub fn is_active_validator(&self) -> bool {
        self.params.enabled
            && self
                .validator_set()
                .get_by_address(self.address())
                .is_some()
    }

    pub fn round_certificate(&self) -> Option<&EnterRoundCertificate<Ctx>> {
        self.driver.round_certificate.as_ref()
    }
}

fn own_non_nil_precommit_extension<'a, Ctx>(
    input: &'a Input<Ctx>,
    address: &Ctx::Address,
    height: &Ctx::Height,
) -> Option<(Round, ValueId<Ctx>, &'a SignedExtension<Ctx>)>
where
    Ctx: Context,
{
    let Input::Vote(vote) = input else {
        return None;
    };
    if vote.height() != *height {
        return None;
    }
    if vote.validator_address() != address {
        return None;
    }
    if vote.vote_type() != VoteType::Precommit {
        return None;
    }
    let NilOrVal::Val(value_id) = vote.value() else {
        return None;
    };
    let extension = vote.extension()?;

    Some((vote.round(), value_id.clone(), extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use malachitebft_core_types::Vote as _;
    use malachitebft_test::{Address, Height, Signature, TestContext, ValueId, Vote};

    fn addr(n: u8) -> Address {
        Address::new([n; 20])
    }

    fn extension(data: &'static [u8]) -> SignedExtension<TestContext> {
        SignedMessage::new(Bytes::from_static(data), Signature::test())
    }

    fn signed(vote: Vote) -> SignedVote<TestContext> {
        SignedVote::new(vote, Signature::test())
    }

    #[test]
    fn own_non_nil_precommit_extension_table() {
        let own = addr(1);
        let other = addr(2);
        let height = Height::new(1);
        let round = Round::new(0);
        let later_round = Round::new(3);
        let value_id = ValueId::new(42);
        let ext = extension(b"ext");

        struct Case {
            name: &'static str,
            input: Input<TestContext>,
            expected: Option<(Round, ValueId)>,
        }

        let cases = [
            Case {
                name: "own non-nil precommit with extension",
                input: Input::Vote(signed(
                    Vote::new_precommit(height, round, NilOrVal::Val(value_id), own)
                        .extend(ext.clone()),
                )),
                expected: Some((round, value_id)),
            },
            Case {
                name: "own non-nil precommit at another round still matches",
                input: Input::Vote(signed(
                    Vote::new_precommit(height, later_round, NilOrVal::Val(value_id), own)
                        .extend(ext.clone()),
                )),
                expected: Some((later_round, value_id)),
            },
            Case {
                name: "non-vote input",
                input: Input::TimeoutElapsed(Timeout::propose(round)),
                expected: None,
            },
            Case {
                name: "different height",
                input: Input::Vote(signed(
                    Vote::new_precommit(Height::new(2), round, NilOrVal::Val(value_id), own)
                        .extend(ext.clone()),
                )),
                expected: None,
            },
            Case {
                name: "other validator",
                input: Input::Vote(signed(
                    Vote::new_precommit(height, round, NilOrVal::Val(value_id), other)
                        .extend(ext.clone()),
                )),
                expected: None,
            },
            Case {
                name: "prevote with extension",
                input: Input::Vote(signed(
                    Vote::new_prevote(height, round, NilOrVal::Val(value_id), own)
                        .extend(ext.clone()),
                )),
                expected: None,
            },
            Case {
                name: "nil precommit with extension",
                input: Input::Vote(signed(
                    Vote::new_precommit(height, round, NilOrVal::Nil, own).extend(ext.clone()),
                )),
                expected: None,
            },
            Case {
                name: "non-nil precommit without extension",
                input: Input::Vote(signed(Vote::new_precommit(
                    height,
                    round,
                    NilOrVal::Val(value_id),
                    own,
                ))),
                expected: None,
            },
        ];

        for case in cases {
            let got = own_non_nil_precommit_extension(&case.input, &own, &height);
            assert_eq!(
                got.map(|(r, v, e)| (r, v, e.clone())),
                case.expected.map(|(r, v)| (r, v, ext.clone())),
                "{}",
                case.name
            );
        }
    }
}

//! Byzantine amnesia state machine.
//!
//! The context-generic [`Amnesia`] struct owns the amnesia state:
//! it tracks the most recently proposed value for a `(height, round)` and
//! decides whether the consensus actor should override a nil prevote with
//! a vote for that value (ignoring the voting lock).
//!
//! Amnesia cannot be implemented at the [`crate::ByzantineNetworkProxy`]
//! level because the consensus engine applies the lock *before* emitting the
//! vote to the network; by that point the vote is already nil. Instead, each
//! integration plugs `Amnesia<Ctx>` into its own prevote-construction
//! path. For the test app, that adapter lives in the
//! `malachitebft_test::byzantine::ByzantineMiddleware` type; downstream
//! contexts embed `Amnesia<Ctx>` into whatever hook their own
//! `Context` impl exposes.

use std::fmt;
use std::sync::Mutex;

use rand::rngs::StdRng;
use tracing::{debug, warn};

use malachitebft_core_types::{Context, Round, ValueId};

use crate::config::{make_rng, Trigger};

/// The cached "last proposed value" slot: `(height, round, value_id)`.
type CachedProposal<Ctx> = (<Ctx as Context>::Height, Round, ValueId<Ctx>);

/// Context-generic Byzantine amnesia state machine.
///
/// Tracks the most recently observed proposed value for a `(height, round)`
/// pair and, when the `ignore_locks` trigger fires, instructs the caller to
/// override a nil prevote with a vote for that value.
///
/// Trait bounds are light: [`malachitebft_core_types::Height`] already
/// provides `Copy + Eq`, and `ValueId<Ctx>` already requires `Clone + Eq`.
/// No additional bounds are needed on `Ctx: Context`.
///
/// # Usage
///
/// Integrations call [`record_proposed_value`](Self::record_proposed_value)
/// from both the local-proposal and incoming-proposal paths, and
/// [`try_override_nil_prevote`](Self::try_override_nil_prevote) from their
/// prevote-construction path. When the latter returns `Some(value_id)`, the
/// caller should build a vote for that value instead of nil.
pub struct Amnesia<Ctx: Context> {
    /// When the attack fires. If `Trigger::Never`, the amnesia is inert and
    /// all helpers short-circuit without touching the mutexes.
    pub ignore_locks: Trigger,
    /// The most recently recorded proposed value for a `(height, round)`.
    /// Mutated from both proposal paths (local + incoming) and consumed on
    /// a successful nil-prevote override.
    current_proposed_value: Mutex<Option<CachedProposal<Ctx>>>,
    /// RNG for evaluating random triggers.
    rng: Mutex<StdRng>,
}

impl<Ctx: Context> Amnesia<Ctx> {
    /// Create a new amnesia tracker with the given trigger and optional RNG seed.
    pub fn new(ignore_locks: Trigger, seed: Option<u64>) -> Self {
        Self {
            ignore_locks,
            current_proposed_value: Mutex::new(None),
            rng: Mutex::new(make_rng(seed)),
        }
    }

    /// Evaluate the `ignore_locks` trigger for `(height, round)`.
    pub fn should_ignore_locks(&self, height: Ctx::Height, round: Round) -> bool {
        self.ignore_locks
            .fires(height, round, &mut self.rng.lock().expect("poisoned rng"))
    }

    /// Cache `value_id` as the most-recently-proposed value for
    /// `(height, round)`. Called from both local-proposal and
    /// incoming-proposal paths; the later call wins.
    ///
    /// When the trigger is `Never` this is a no-op.
    pub fn record_proposed_value(&self, height: Ctx::Height, round: Round, value_id: ValueId<Ctx>) {
        if !self.ignore_locks.is_set() {
            return;
        }
        *self.current_proposed_value.lock().expect("poisoned mutex") =
            Some((height, round, value_id));
    }

    /// Decide whether a nil prevote at `(height, round)` should be overridden
    /// with a vote for the cached proposed value.
    ///
    /// Returns `Some(value_id)` on override (the cached value is consumed),
    /// `None` otherwise. All logging happens here so every caller gets a
    /// consistent trail.
    pub fn try_override_nil_prevote(
        &self,
        height: Ctx::Height,
        round: Round,
    ) -> Option<ValueId<Ctx>> {
        if !self.should_ignore_locks(height, round) {
            return None;
        }

        let mut guard = self.current_proposed_value.lock().expect("poisoned mutex");

        match guard.as_ref() {
            Some((h, r, _)) if *h == height && *r == round => {
                let (_, _, vid) = guard.take().expect("just matched Some");
                warn!(%height, %round, "BYZANTINE AMNESIA: Overriding nil prevote with value (ignoring lock)");
                Some(vid)
            }
            Some((h, r, _)) => {
                debug!(
                    %height, %round, stored_height = %h, stored_round = %r,
                    "BYZANTINE AMNESIA: Stored value is from a different height and/or round, not overriding"
                );
                None
            }
            None => {
                debug!(%height, %round, "BYZANTINE AMNESIA: Trigger fired but no proposed value cached, not overriding");
                None
            }
        }
    }
}

impl<Ctx: Context> fmt::Debug for Amnesia<Ctx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Amnesia")
            .field("ignore_locks", &self.ignore_locks)
            .finish_non_exhaustive()
    }
}

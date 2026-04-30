//! Byzantine middleware for the test app's [`Middleware`] trait.
//!
//! [`ByzantineMiddleware`] wraps an inner middleware and overrides nil prevote
//! construction to simulate amnesia attacks (ignoring voting locks).
//!
//! When `ignore_locks` is triggered, the middleware tracks proposed values via
//! the [`on_propose_value`](Middleware::on_propose_value) and
//! [`get_validity`](Middleware::get_validity) callbacks and overrides nil
//! prevotes to vote for the most recently proposed value, but only when the
//! stored `(height, round)` matches the current prevote step.
//!
//! When `force_precommit_nil` is triggered, the middleware rewrites non-nil
//! precommits into nil precommits at the point the driver emits the vote.
//! This is done in the middleware rather than at the network proxy on purpose:
//! rewriting the vote before it leaves the driver prevents the precommit-for-
//! Val side effects in `handle/driver.rs` (restreaming the proposal and
//! publishing a polka certificate via liveness), which would otherwise help
//! peers that are supposed to be starved of information in a test scenario.
//!
//! IMPORTANT: the middleware's `new_precommit` is also used by certificate
//! verification to reconstruct precommit votes for *other* validators'
//! signatures (see `verify_commit_signature` / `verify_polka_signature`).
//! Rewriting those reconstructions would break signature verification. We
//! therefore gate the rewrite on the vote's `address` matching the node's own
//! address (passed in at construction time).

use eyre::Result;
use rand::rngs::StdRng;
use std::fmt;
use std::sync::{Arc, Mutex};

use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{CommitCertificate, LinearTimeouts, NilOrVal, Round, Validity};
use malachitebft_test::middleware::Middleware;
use malachitebft_test::{
    Address, Genesis, Height, Proposal, TestContext, ValidatorSet, Value, ValueId, Vote,
};
use tracing::{debug, warn};

use crate::config::{make_rng, Trigger};

/// A middleware that simulates Byzantine amnesia by ignoring voting locks.
///
/// When the `ignore_locks` trigger fires, this middleware tracks the most
/// recently proposed value via [`on_propose_value`](Middleware::on_propose_value)
/// (local proposals) and [`get_validity`](Middleware::get_validity)
/// (incoming proposals), and overrides nil prevotes to vote for that value
/// instead, but only when the stored `(height, round)` matches the current
/// prevote step, preventing stale values from leaking across heights/rounds.
///
/// When the `force_precommit_nil` trigger fires, the middleware rewrites
/// non-nil precommits into nil precommits on the output of the driver.
/// Rewriting at this level (rather than at the network proxy) also suppresses
/// the downstream side effects of a precommit-for-value: the driver will not
/// observe a `(Precommit, Val)` emission and therefore will not restream the
/// proposal nor publish a polka certificate via liveness. This is important
/// for tests that deliberately starve other nodes of catch-up information.
///
/// All other middleware methods delegate to the inner middleware.
///
/// # Usage
///
/// ```rust,ignore
/// let inner = Arc::new(DefaultMiddleware);
/// let byzantine = ByzantineMiddleware::new(Trigger::Always, Trigger::Never, inner, None);
/// let ctx = TestContext::with_middleware(Arc::new(byzantine));
/// ```
pub struct ByzantineMiddleware {
    /// When to ignore voting locks (amnesia attack).
    pub ignore_locks: Trigger,
    /// When to rewrite non-nil precommits into nil precommits.
    pub force_precommit_nil: Trigger,
    /// The inner middleware to delegate to for non-Byzantine behavior.
    pub inner: Arc<dyn Middleware>,
    /// The node's own validator address. `new_precommit` only rewrites when
    /// the vote being constructed is for this address, so certificate-
    /// verification reconstructions for other validators are left intact.
    pub self_address: Address,
    /// Tracks the most recently proposed value ID for a `(Height, Round)`,
    /// captured via `get_validity` and `on_propose_value`. When amnesia is
    /// active (`ignore_locks` fires), `new_prevote` votes for this value
    /// instead of the locked one.
    current_proposed_value: Mutex<Option<(Height, Round, ValueId)>>,
    /// RNG for evaluating random triggers.
    rng: Mutex<StdRng>,
}

impl ByzantineMiddleware {
    /// Create a new `ByzantineMiddleware`.
    pub fn new(
        ignore_locks: Trigger,
        force_precommit_nil: Trigger,
        inner: Arc<dyn Middleware>,
        self_address: Address,
        seed: Option<u64>,
    ) -> Self {
        Self {
            ignore_locks,
            force_precommit_nil,
            inner,
            self_address,
            current_proposed_value: Mutex::new(None),
            rng: Mutex::new(make_rng(seed)),
        }
    }
}

impl fmt::Debug for ByzantineMiddleware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ByzantineMiddleware")
            .field("ignore_locks", &self.ignore_locks)
            .field("force_precommit_nil", &self.force_precommit_nil)
            .field("inner", &self.inner)
            .finish()
    }
}

impl ByzantineMiddleware {
    /// Evaluate the `ignore_locks` trigger for the given height and round.
    fn should_ignore_locks(&self, height: Height, round: Round) -> bool {
        self.ignore_locks
            .fires(height, round, &mut self.rng.lock().expect("poisoned rng"))
    }

    /// Evaluate the `force_precommit_nil` trigger for the given height and round.
    fn should_force_precommit_nil(&self, height: Height, round: Round) -> bool {
        self.force_precommit_nil
            .fires(height, round, &mut self.rng.lock().expect("poisoned rng"))
    }
}

impl Middleware for ByzantineMiddleware {
    fn get_validator_set(
        &self,
        ctx: &TestContext,
        current_height: Height,
        height: Height,
        genesis: &Genesis,
    ) -> Option<ValidatorSet> {
        self.inner
            .get_validator_set(ctx, current_height, height, genesis)
    }

    fn get_timeouts(
        &self,
        ctx: &TestContext,
        current_height: Height,
        height: Height,
    ) -> Option<LinearTimeouts> {
        self.inner.get_timeouts(ctx, current_height, height)
    }

    fn new_proposal(
        &self,
        ctx: &TestContext,
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        self.inner
            .new_proposal(ctx, height, round, value, pol_round, address)
    }

    fn new_prevote(
        &self,
        ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        if self.should_ignore_locks(height, round) {
            if let NilOrVal::Nil = &value_id {
                let value_id = {
                    let mut guard = self.current_proposed_value.lock().expect("poisoned mutex");
                    match guard.as_ref() {
                        Some((h, r, _)) if *h == height && *r == round => {
                            let (_, _, vid) = guard.take().expect("just matched Some");
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
                }; // guard dropped here
                if let Some(vid) = value_id {
                    warn!(%height, %round, "BYZANTINE AMNESIA: Overriding nil prevote with value (ignoring lock)");
                    return self
                        .inner
                        .new_prevote(ctx, height, round, NilOrVal::Val(vid), address);
                }
            }
        }

        self.inner
            .new_prevote(ctx, height, round, value_id, address)
    }

    fn new_precommit(
        &self,
        ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        if address == self.self_address
            && matches!(value_id, NilOrVal::Val(_))
            && self.should_force_precommit_nil(height, round)
        {
            warn!(%height, %round, "BYZANTINE: Forcing precommit nil (rewriting non-nil precommit)");
            return self
                .inner
                .new_precommit(ctx, height, round, NilOrVal::Nil, address);
        }

        self.inner
            .new_precommit(ctx, height, round, value_id, address)
    }

    fn on_propose_value(
        &self,
        ctx: &TestContext,
        proposed_value: &mut LocallyProposedValue<TestContext>,
        reproposal: bool,
    ) {
        // Always cache the proposed value so that `new_prevote` can use it
        // when the trigger fires. Evaluating the trigger here independently
        // would cause random triggers to fire with probability p² instead of p.
        if self.ignore_locks.is_set() {
            let vid = proposed_value.value.id();
            *self.current_proposed_value.lock().expect("poisoned mutex") =
                Some((proposed_value.height, proposed_value.round, vid));
        }

        self.inner.on_propose_value(ctx, proposed_value, reproposal)
    }

    fn get_validity(
        &self,
        ctx: &TestContext,
        height: Height,
        round: Round,
        value: &Value,
    ) -> Validity {
        // Always cache the proposed value (see on_propose_value comment).
        if self.ignore_locks.is_set() {
            let vid = value.id();
            *self.current_proposed_value.lock().expect("poisoned mutex") =
                Some((height, round, vid));
        }

        self.inner.get_validity(ctx, height, round, value)
    }

    fn on_commit(
        &self,
        ctx: &TestContext,
        certificate: &CommitCertificate<TestContext>,
        proposal: &ProposedValue<TestContext>,
    ) -> Result<()> {
        self.inner.on_commit(ctx, certificate, proposal)
    }
}

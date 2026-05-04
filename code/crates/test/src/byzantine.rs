//! Byzantine [`Middleware`] adapter for the test app.
//!
//! [`ByzantineMiddleware`] wraps an inner [`Middleware`] and intercepts vote
//! construction to simulate two Byzantine attacks:
//!
//! - **Amnesia** (`ignore_locks`): delegates to
//!   [`Amnesia<TestContext>`] and overrides nil prevotes with a vote
//!   for the most recently observed proposal value (ignoring the voting lock).
//! - **Force precommit nil** (`force_precommit_nil`): rewrites this node's
//!   non-nil precommits into nil precommits at the point the driver emits the
//!   vote. Rewriting at this level (rather than at the network proxy) also
//!   suppresses the downstream side effects of a precommit-for-value
//!   (`handle/driver.rs` would otherwise restream the proposal and publish a
//!   polka certificate via liveness, helping peers that are supposed to be
//!   starved of information in a test scenario).
//!
//! All other trait methods pass through to the inner middleware.
//!
//! This adapter is specific to `TestContext`; other integrations embed
//! [`Amnesia<Ctx>`] directly in their own prevote-construction
//! path. See the crate-level docs of [`malachitebft_engine_byzantine`] for
//! the generic pattern.
//!
//! IMPORTANT: `new_precommit` is also used by certificate verification to
//! reconstruct precommit votes for *other* validators' signatures (see
//! `verify_commit_signature` / `verify_polka_signature`). Rewriting those
//! reconstructions would break signature verification. The rewrite is
//! therefore gated on the vote's `address` matching the node's own address
//! (passed in at construction time).

use std::sync::{Arc, Mutex};

use eyre::Result;
use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{CommitCertificate, LinearTimeouts, NilOrVal, Round, Validity};
use malachitebft_engine_byzantine::config::make_rng;
use malachitebft_engine_byzantine::{Amnesia, Trigger};
use rand::rngs::StdRng;
use tracing::warn;

use crate::middleware::Middleware;
use crate::{Address, Genesis, Height, Proposal, TestContext, ValidatorSet, Value, ValueId, Vote};

/// A [`Middleware`] that simulates Byzantine amnesia and force-precommit-nil
/// attacks for the test app.
///
/// # Usage
///
/// ```rust,ignore
/// let inner = Arc::new(DefaultMiddleware);
/// let byzantine = ByzantineMiddleware::new(
///     Trigger::Always, Trigger::Never, inner, self_address, None,
/// );
/// let ctx = TestContext::with_middleware(Arc::new(byzantine));
/// ```
pub struct ByzantineMiddleware {
    /// Context-generic amnesia core (drives `ignore_locks`).
    pub amnesia: Amnesia<TestContext>,
    /// When to rewrite non-nil precommits into nil precommits.
    pub force_precommit_nil: Trigger,
    /// The inner middleware to delegate to for non-Byzantine behavior.
    pub inner: Arc<dyn Middleware>,
    /// The node's own validator address. `new_precommit` only rewrites when
    /// the vote being constructed is for this address, so certificate-
    /// verification reconstructions for other validators are left intact.
    pub self_address: Address,
    /// RNG for evaluating `force_precommit_nil`'s random triggers.
    /// `Amnesia` owns its own RNG for `ignore_locks`.
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
            amnesia: Amnesia::new(ignore_locks, seed),
            force_precommit_nil,
            inner,
            self_address,
            rng: Mutex::new(make_rng(seed)),
        }
    }

    fn should_force_precommit_nil(&self, height: Height, round: Round) -> bool {
        self.force_precommit_nil
            .fires(height, round, &mut self.rng.lock().expect("poisoned rng"))
    }
}

impl std::fmt::Debug for ByzantineMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByzantineMiddleware")
            .field("amnesia", &self.amnesia)
            .field("force_precommit_nil", &self.force_precommit_nil)
            .field("inner", &self.inner)
            .finish()
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
        if let NilOrVal::Nil = value_id {
            if let Some(vid) = self.amnesia.try_override_nil_prevote(height, round) {
                return self
                    .inner
                    .new_prevote(ctx, height, round, NilOrVal::Val(vid), address);
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
        // No trigger guard here — the trigger fires once per prevote (inside
        // `try_override_nil_prevote`); evaluating it a second time at this
        // call site would produce an effective firing probability of p²
        // instead of p for a random trigger.
        self.amnesia.record_proposed_value(
            proposed_value.height,
            proposed_value.round,
            proposed_value.value.id(),
        );

        self.inner.on_propose_value(ctx, proposed_value, reproposal)
    }

    fn get_validity(
        &self,
        ctx: &TestContext,
        height: Height,
        round: Round,
        value: &Value,
    ) -> Validity {
        self.amnesia
            .record_proposed_value(height, round, value.id());
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

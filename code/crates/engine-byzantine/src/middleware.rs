//! Byzantine middleware for the test app's [`Middleware`] trait.
//!
//! [`ByzantineMiddleware`] wraps an inner middleware and overrides vote
//! construction to simulate amnesia attacks (ignoring voting locks).
//!
//! When `ignore_locks` is enabled, the middleware tracks proposed values via
//! the [`get_validity`](Middleware::get_validity) callback and overrides nil
//! prevotes to vote for the most recently proposed value in the current round.

use std::fmt;
use std::sync::{Arc, Mutex};

use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{CommitCertificate, LinearTimeouts, NilOrVal, Round, Validity};
use malachitebft_test::middleware::Middleware;
use malachitebft_test::{
    Address, Genesis, Height, Proposal, TestContext, ValidatorSet, Value, ValueId, Vote,
};

/// A middleware that simulates Byzantine amnesia by ignoring voting locks.
///
/// When `ignore_locks` is `true`, this middleware tracks the most recently
/// proposed value via [`get_validity`] and overrides nil prevotes to vote for
/// that value instead. All other middleware methods delegate to the inner
/// middleware.
///
/// # Usage
///
/// ```rust,ignore
/// let inner = Arc::new(DefaultMiddleware);
/// let byzantine = ByzantineMiddleware::new(true, inner);
/// let ctx = TestContext::with_middleware(Arc::new(byzantine));
/// ```
pub struct ByzantineMiddleware {
    /// Whether to ignore voting locks (amnesia attack).
    pub ignore_locks: bool,
    /// The inner middleware to delegate to for non-Byzantine behavior.
    pub inner: Arc<dyn Middleware>,
    /// Tracks the most recently proposed value ID for the current round,
    /// captured via `get_validity`.
    current_proposed_value: Mutex<Option<ValueId>>,
}

impl ByzantineMiddleware {
    /// Create a new `ByzantineMiddleware`.
    pub fn new(ignore_locks: bool, inner: Arc<dyn Middleware>) -> Self {
        Self {
            ignore_locks,
            inner,
            current_proposed_value: Mutex::new(None),
        }
    }
}

impl fmt::Debug for ByzantineMiddleware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ByzantineMiddleware")
            .field("ignore_locks", &self.ignore_locks)
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
        if self.ignore_locks {
            if let NilOrVal::Nil = &value_id {
                // The state machine decided nil (likely due to lock on a different value).
                // Override with the most recently proposed value if we have one.
                let stored = self.current_proposed_value.lock().unwrap().take();
                if let Some(vid) = stored {
                    tracing::warn!(
                        %height, %round,
                        "BYZANTINE AMNESIA: Overriding nil prevote with value (ignoring lock)"
                    );
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
        self.inner
            .new_precommit(ctx, height, round, value_id, address)
    }

    fn on_propose_value(
        &self,
        ctx: &TestContext,
        proposed_value: &mut LocallyProposedValue<TestContext>,
        reproposal: bool,
    ) {
        // Track the proposed value for potential amnesia override.
        if self.ignore_locks {
            let vid = proposed_value.value.id();
            *self.current_proposed_value.lock().unwrap() = Some(vid);
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
        // Track the proposed value for potential amnesia override.
        // This is called when we receive a proposed value from another node,
        // giving us a chance to capture the value ID before `new_prevote`.
        if self.ignore_locks {
            let vid = value.id();
            *self.current_proposed_value.lock().unwrap() = Some(vid);
        }

        self.inner.get_validity(ctx, height, round, value)
    }

    fn on_commit(
        &self,
        ctx: &TestContext,
        certificate: &CommitCertificate<TestContext>,
        proposal: &ProposedValue<TestContext>,
    ) -> Result<(), eyre::Report> {
        self.inner.on_commit(ctx, certificate, proposal)
    }
}

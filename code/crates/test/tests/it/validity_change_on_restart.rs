use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::info;

use arc_malachitebft_test::middleware::Middleware;
use arc_malachitebft_test::{Height, TestContext, Value};
use malachitebft_config::ValuePayload;
use malachitebft_core_types::{Round, Validity};
use malachitebft_engine::util::events::Event;

use crate::{HandlerResult, TestBuilder, TestParams};

/// Middleware that marks a specific height as Invalid before crash,
/// and Valid after restart (simulating changed validation logic)
#[derive(Clone, Debug)]
struct InvalidAtHeightMiddleware {
    target_height: Height,
    mark_invalid: Arc<AtomicBool>,
}

impl InvalidAtHeightMiddleware {
    fn new(target_height: Height) -> Self {
        Self {
            target_height,
            mark_invalid: Arc::new(AtomicBool::new(true)),
        }
    }

    fn set_valid_after_restart(&self) {
        self.mark_invalid.store(false, Ordering::SeqCst);
    }
}

impl Middleware for InvalidAtHeightMiddleware {
    fn get_validity(
        &self,
        _ctx: &TestContext,
        height: Height,
        _round: Round,
        value: &Value,
    ) -> Validity {
        if height == self.target_height && self.mark_invalid.load(Ordering::SeqCst) {
            info!(
                %height,
                value_id = %value.id(),
                "Marking value as Invalid (will be stored in WAL with Invalid)"
            );
            Validity::Invalid
        } else {
            if height == self.target_height {
                info!(
                    %height,
                    value_id = %value.id(),
                    "Marking value as Valid (after restart, sync will provide with cert)"
                );
            }
            Validity::Valid
        }
    }
}

/// Test that a node can recover via sync after crashing with an Invalid value
/// Without the fix in `full_proposal.rs`, the node would keep Invalid validity
/// and fail to progress after restart.
#[tokio::test]
async fn node_with_invalid_value_recovers_via_sync() {
    #[derive(Clone, Debug, Default)]
    struct State {
        has_changed_middleware: bool,
    }

    const CRASH_HEIGHT: u64 = 3;
    const FINAL_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<State>::new();

    // Three normal nodes that consider all values valid (75% voting power)
    test.add_node().with_voting_power(25).start().success();
    test.add_node().with_voting_power(25).start().success();
    test.add_node().with_voting_power(25).start().success();

    // Fourth node with middleware that marks height 3 as Invalid
    let middleware = InvalidAtHeightMiddleware::new(Height::new(CRASH_HEIGHT));
    let middleware_clone = middleware.clone();

    test.add_node()
        .with_voting_power(25)
        .with_middleware(middleware)
        .start()
        .wait_until(CRASH_HEIGHT)
        // First handler detects Invalid value and triggers crash
        .on_event(move |event, _state| match event {
            Event::ReceivedProposedValue(value, _) if value.height.as_u64() == CRASH_HEIGHT => {
                if value.validity == Validity::Invalid {
                    info!(
                        "✓ Value marked INVALID at height {} - will crash in 10ms",
                        value.height
                    );
                    Ok(HandlerResult::ContinueTest)
                } else {
                    Ok(HandlerResult::WaitForNextEvent)
                }
            }
            _ => Ok(HandlerResult::WaitForNextEvent),
        })
        // TODO - fix this to allow sync before crash
        // Crash after 10ms to prevent other events to trigger second handler
        .crash_after(Duration::from_millis(10))
        // Restart after other nodes have moved on
        .restart_after(Duration::from_secs(2))
        // Second handler: change middleware flag on first event after restart, then stop
        .on_event(move |_event, state| {
            if !state.has_changed_middleware {
                middleware_clone.set_valid_after_restart();
                info!("✓ After restart - middleware now returns Valid for height 3");
                state.has_changed_middleware = true;
            }
            // Return ContinueTest to stop consuming events, allowing wait_until to work
            Ok(HandlerResult::ContinueTest)
        })
        // Timeline:
        // - Before crash: Value stored as Invalid in WAL
        // - After restart: WAL replay with Invalid, middleware returns Valid for sync values
        // - Without the fix: panic at decide.rs:64 (Invalid vs Valid mismatch)
        // - With the fix: keeper updates Invalid→Valid, continues successfully
        .wait_until(CRASH_HEIGHT + 1)
        // Verify we can continue to make progress
        .wait_until(FINAL_HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(20),
            TestParams {
                value_payload: ValuePayload::ProposalAndParts,
                enable_value_sync: true, // Critical: sync must be enabled for recovery
                ..TestParams::default()
            },
        )
        .await
}

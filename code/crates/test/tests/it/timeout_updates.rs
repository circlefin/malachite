use std::time::Duration;

use arc_malachitebft_test::middleware::Middleware;
use arc_malachitebft_test::{Height, LinearTimeouts, TestContext};

use crate::TestBuilder;

/// A middleware that changes timeouts at specific heights
#[derive(Copy, Clone, Debug)]
struct TimeoutChangingMiddleware {
    change_height: u64,
    new_propose_timeout: Duration,
}

impl Middleware for TimeoutChangingMiddleware {
    fn get_timeouts(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        height: Height,
    ) -> Option<LinearTimeouts> {
        if height.as_u64() >= self.change_height {
            Some(LinearTimeouts {
                propose: self.new_propose_timeout,
                propose_delta: Duration::from_millis(500),
                prevote: Duration::from_secs(1),
                prevote_delta: Duration::from_millis(500),
                precommit: Duration::from_secs(1),
                precommit_delta: Duration::from_millis(500),
                rebroadcast: self.new_propose_timeout + Duration::from_secs(2),
            })
        } else {
            None
        }
    }
}

/// Test that nodes can change timeouts between heights and still reach consensus
#[tokio::test]
async fn change_timeouts_between_heights() {
    const HEIGHT: u64 = 5;
    const CHANGE_HEIGHT: u64 = 3;

    let middleware = TimeoutChangingMiddleware {
        change_height: CHANGE_HEIGHT,
        new_propose_timeout: Duration::from_millis(500), // Shorter timeout
    };

    let mut test = TestBuilder::<()>::new();

    // Create 3 nodes that all change their timeouts at height 3
    for _ in 0..3 {
        test.add_node()
            .with_middleware(middleware)
            .start()
            .wait_until(HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(60)).await
}

/// Test that nodes with different timeouts can still reach consensus
#[tokio::test]
async fn different_timeouts_per_node() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: uses very short timeouts from height 2 onwards
    let short_timeout_middleware = TimeoutChangingMiddleware {
        change_height: 2,
        new_propose_timeout: Duration::from_millis(200),
    };
    test.add_node()
        .with_middleware(short_timeout_middleware)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 2: uses default timeouts (no change)
    test.add_node().start().wait_until(HEIGHT).success();

    // Node 3: uses longer timeouts from height 3 onwards
    let long_timeout_middleware = TimeoutChangingMiddleware {
        change_height: 3,
        new_propose_timeout: Duration::from_secs(5),
    };
    test.add_node()
        .with_middleware(long_timeout_middleware)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build().run(Duration::from_secs(90)).await
}

/// A middleware that uses extremely short timeouts to test that they're actually being applied
#[derive(Copy, Clone, Debug)]
struct VeryShortTimeouts;

impl Middleware for VeryShortTimeouts {
    fn get_timeouts(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        _height: Height,
    ) -> Option<LinearTimeouts> {
        Some(LinearTimeouts {
            propose: Duration::from_millis(100),
            propose_delta: Duration::from_millis(50),
            prevote: Duration::from_millis(50),
            prevote_delta: Duration::from_millis(50),
            precommit: Duration::from_millis(50),
            precommit_delta: Duration::from_millis(50),
            rebroadcast: Duration::from_millis(300),
        })
    }
}

/// Test that consensus works with very short timeouts (verifies timeouts are actually applied)
#[tokio::test]
async fn very_short_timeouts() {
    const HEIGHT: u64 = 3;

    let mut test = TestBuilder::<()>::new();

    for _ in 0..3 {
        test.add_node()
            .with_middleware(VeryShortTimeouts)
            .start()
            .wait_until(HEIGHT)
            .success();
    }

    // If timeouts are properly applied, this should complete quickly
    // Give it 30 seconds which should be plenty for 3 heights with short timeouts
    test.build().run(Duration::from_secs(30)).await
}

/// A middleware that gradually decreases timeouts with each height
#[derive(Copy, Clone, Debug)]
struct GraduallyDecreasingTimeouts;

impl Middleware for GraduallyDecreasingTimeouts {
    fn get_timeouts(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        height: Height,
    ) -> Option<LinearTimeouts> {
        // Start with 3s propose timeout, decrease by 200ms per height, minimum 500ms
        let height_num = height.as_u64();
        let propose_millis = 3000u64.saturating_sub(height_num * 200).max(500);

        Some(LinearTimeouts {
            propose: Duration::from_millis(propose_millis),
            propose_delta: Duration::from_millis(100),
            prevote: Duration::from_millis(500),
            prevote_delta: Duration::from_millis(100),
            precommit: Duration::from_millis(500),
            precommit_delta: Duration::from_millis(100),
            rebroadcast: Duration::from_secs(2),
        })
    }
}

/// Test that timeouts can be adjusted gradually over many heights
#[tokio::test]
async fn gradually_changing_timeouts() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    for _ in 0..3 {
        test.add_node()
            .with_middleware(GraduallyDecreasingTimeouts)
            .start()
            .wait_until(HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(90)).await
}

use std::time::Duration;

use malachitebft_config::VoteSyncMode;

use crate::{TestBuilder, TestParams};

// NOTE: These tests are very similar to the Sync tests, with the difference that
//       all nodes have the same voting power and therefore get stuck when one of them dies.

#[tokio::test]
pub async fn crash_restart_from_start() {
    const CRASH_HEIGHT: u64 = 4;
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        // Wait until the node reaches height 4...
        .wait_until(CRASH_HEIGHT)
        // ...then kill it
        .crash()
        // Reset the database so that the node has to do Sync from height 1
        .reset_db()
        // After that, it waits 5 seconds before restarting the node
        .restart_after(Duration::from_secs(5))
        // Request vote set from other nodes
        .expect_vote_set_request(CRASH_HEIGHT)
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_value_sync: true, // Enable ValueSync to allow node to catch up to latest height
                vote_sync_mode: Some(VoteSyncMode::RequestResponse),
                timeout_step: Duration::from_secs(5),
                ..TestParams::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn start_late() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node()
        .start_after(1, Duration::from_secs(10))
        // Expect a vote set request for height 1
        .expect_vote_set_request(1)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true, // Enable ValueSync to allow node to catch up to latest height
                vote_sync_mode: Some(VoteSyncMode::RequestResponse),
                timeout_step: Duration::from_secs(5),
                ..Default::default()
            },
        )
        .await
}

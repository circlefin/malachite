use std::time::Duration;

use malachitebft_config::{ValuePayload, VoteSyncMode};

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
        .expect_vote_rebroadcast(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .expect_vote_rebroadcast(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        // Wait until the node reaches height 4...
        .wait_until(4)
        // ...then kill it
        .crash()
        // Reset the database so that the node has to do Sync from height 1
        .reset_db()
        // After that, it waits 5 seconds before restarting the node
        .restart_after(Duration::from_secs(5))
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                // Enable Sync to allow the node to catch up to the latest height
                enable_value_sync: true,
                vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
                timeout_step: Duration::from_secs(5),
                value_payload: ValuePayload::PartsOnly,
                ..TestParams::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn crash_restart_from_latest() {
    const HEIGHT: u64 = 10;
    const CRASH_HEIGHT: u64 = 4;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .expect_vote_rebroadcast(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .expect_vote_rebroadcast(CRASH_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        .wait_until(CRASH_HEIGHT)
        .crash()
        // We do not reset the database so that the node can restart from the latest height
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: false,
                vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn start_late() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .start()
        .wait_until(1)
        .expect_vote_rebroadcast(1)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start()
        .wait_until(1)
        .expect_vote_rebroadcast(1)
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: false,
                vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
                ..Default::default()
            },
        )
        .await
}

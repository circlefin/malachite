use std::time::Duration;

use informalsystems_malachitebft_starknet_test::{init_logging, TestBuilder, TestParams};
use malachitebft_config::ValuePayload;

// NOTE: These tests are very similar to the Sync tests, with the difference that
//       all nodes have the same voting power and therefore get stuck when one of them dies.

pub async fn crash_restart_from_start(params: TestParams) {
    init_logging(module_path!());

    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();

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
        // Expect a vote set request for height 4
        .expect_vote_set_request(4)
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    test.build()
        .run_with_custom_config(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_sync: true, // Enable Sync
                timeout_step: Duration::from_secs(5),
                ..params
            },
        )
        .await
}

#[tokio::test]
pub async fn crash_restart_from_start_parts_only() {
    let params = TestParams {
        value_payload: ValuePayload::PartsOnly,
        ..Default::default()
    };

    crash_restart_from_start(params).await
}

#[tokio::test]
pub async fn crash_restart_from_start_proposal_only() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..Default::default()
    };

    crash_restart_from_start(params).await
}

#[tokio::test]
pub async fn crash_restart_from_start_proposal_and_parts() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..Default::default()
    };

    crash_restart_from_start(params).await
}

#[tokio::test]
pub async fn crash_restart_from_latest() {
    init_logging(module_path!());

    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node()
        .start()
        .wait_until(2)
        .crash()
        // We do not reset the database so that the node can restart from the latest height
        .restart_after(Duration::from_secs(5))
        // Expect a vote set request for height 2
        .expect_vote_set_request(2)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_custom_config(
            Duration::from_secs(60),
            TestParams {
                enable_sync: true,
                timeout_step: Duration::from_secs(5),
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn start_late() {
    init_logging(module_path!());

    const HEIGHT: u64 = 5;
    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT * 2).success();
    test.add_node().start().wait_until(HEIGHT * 2).success();
    test.add_node()
        .start_after(1, Duration::from_secs(10))
        // Expect a vote set request for height 1
        .expect_vote_set_request(1)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_custom_config(
            Duration::from_secs(60),
            TestParams {
                enable_sync: true,
                timeout_step: Duration::from_secs(5),
                ..Default::default()
            },
        )
        .await
}

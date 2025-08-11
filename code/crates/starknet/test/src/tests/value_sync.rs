use std::time::Duration;

use bytesize::ByteSize;

use crate::{TestBuilder, TestParams};

#[tokio::test]
pub async fn crash_restart_from_start() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1 starts with 10 voting power.
    test.add_node()
        .with_voting_power(10)
        .start()
        // Wait until it reaches height 10
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    // Node 2 starts with 10 voting power, in parallel with node 1 and with the same behaviour
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3 starts with 5 voting power, in parallel with node 1 and 2.
    test.add_node()
        .with_voting_power(5)
        .start()
        // Wait until the node reaches height 2...
        .wait_until(2)
        // ...and then kills it
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
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_value_sync: true, // Enable Sync
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn crash_restart_from_latest() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(2)
        .crash()
        // We do not reset the database so that the node can restart from the latest height
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn aggressive_pruning() {
    const HEIGHT: u64 = 15;

    let mut test = TestBuilder::<()>::new();

    // Node 1 starts with 10 voting power.
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(2)
        .crash()
        .reset_db()
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_value_sync: true, // Enable Sync
                max_retain_blocks: 10,   // Prune blocks older than 10
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn start_late() {
    const HEIGHT: u64 = 3;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn start_late_parallel_requests_with_batching() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(0)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 2,
                batch_size: 2,
                block_size: ByteSize::kib(1),
                ..Default::default()
            },
        )
        .await
}

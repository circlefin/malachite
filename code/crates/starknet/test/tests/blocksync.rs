#![allow(unused_crate_dependencies)]

use std::time::Duration;

use malachite_starknet_test::{Test, TestNode, TestParams};

#[tokio::test]
pub async fn crash_restart() {
    const HEIGHT: u64 = 10;

    // Node 1 starts with 10 voting power.
    let n1 = TestNode::new(1)
        .vp(10)
        .start()
        // Wait until it reaches height 10
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    // Node 2 starts with 10 voting power, in parallel with node 1 and with the same behaviour
    let n2 = TestNode::new(2).vp(10).start().wait_until(HEIGHT).success();

    // Node 3 starts with 5 voting power, in parallel with node 1 and 2.
    let n3 = TestNode::new(3)
        .vp(5)
        .start()
        // Then the test runner waits until it reaches height 2...
        .wait_until(2)
        // ...and kills the node!
        .crash()
        // After that, it waits 5 seconds before restarting the node
        .restart_after(Duration::from_secs(5))
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    Test::new([n1, n2, n3])
        .run_with_custom_config(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_blocksync: true, // Enable BlockSync
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn aggressive_pruning() {
    const HEIGHT: u64 = 15;

    // Node 1 starts with 10 voting power.
    let n1 = TestNode::new(1).vp(10).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).vp(10).start().wait_until(HEIGHT).success();

    let n3 = TestNode::new(3)
        .vp(5)
        .start()
        .wait_until(2)
        .crash()
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    Test::new([n1, n2, n3])
        .run_with_custom_config(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_blocksync: true, // Enable BlockSync
                max_retain_blocks: 10,  // Prune blocks older than 10
                ..Default::default()
            },
        )
        .await
}

// TODO: Enable this test once we can start the network without everybody being online
// #[tokio::test]
// pub async fn blocksync_start_late() {
//     const HEIGHT: u64 = 5;
//
//     let n1 = TestNode::new(1)
//         .voting_power(10)
//         .start(1)
//         .wait_until(HEIGHT * 2)
//         .success();
//
//     let n2 = TestNode::new(2)
//         .voting_power(10)
//         .start(1)
//         .wait_until(HEIGHT * 2)
//         .success();
//
//     let n3 = TestNode::new(3)
//         .voting_power(5)
//         .start_after(1, Duration::from_secs(10))
//         .wait_until(HEIGHT)
//         .success();
//
//     Test::new([n1, n2, n3])
//         .run_with_custom_config(
//             Duration::from_secs(30),
//             TestParams {
//                 enable_blocksync: true,
//                 ..Default::default()
//             },
//         )
//         .await
// }
//

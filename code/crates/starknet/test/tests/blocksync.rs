#![allow(unused_crate_dependencies)]

use std::time::Duration;

use malachite_starknet_test::{App, Test, TestNode, TestParams};

#[tokio::test]
pub async fn crash_restart() {
    const HEIGHT: u64 = 10;

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
            App::Starknet,
            Duration::from_secs(30),
            TestParams {
                enable_blocksync: true,
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
//             App::Starknet,
//             Duration::from_secs(30),
//             TestParams {
//                 enable_blocksync: true,
//                 ..Default::default()
//             },
//         )
//         .await
// }
//

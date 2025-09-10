use std::time::Duration;

use crate::{TestBuilder, TestParams};

#[tokio::test]
pub async fn proposer_fails_to_start() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node().with_voting_power(1).success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn one_node_fails_to_start() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node().with_voting_power(1).success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn proposer_crashes_at_height_2() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(1)
        .start()
        .wait_until(2)
        .crash()
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_3() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(1)
        .start()
        .wait_until(3)
        .crash()
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn validators_restart_at_different_heights_discovery_disabled() {
    const HEIGHT: u64 = 8;

    let mut test = TestBuilder::<()>::new();

    // Node 1: validator restarts at height 3
    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(3)
        .crash()
        .restart_after(Duration::from_secs(2))
        .wait_until(HEIGHT)
        .success();

    // Node 2: validator restarts at height 5
    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(5)
        .crash()
        .restart_after(Duration::from_secs(2))
        .wait_until(HEIGHT)
        .success();

    // Node 3: validator excluded from persistent peers (others don't connect to it)
    // but it has nodes 1 and 2 as persistent peers and must reconnect to them
    test.add_node()
        .with_voting_power(1)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(45),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 1,
                enable_discovery: false,
                // Node 3 is excluded from persistent peers of nodes 1 and 2
                exclude_from_persistent_peers: vec![3],
                ..Default::default()
            },
        )
        .await
}

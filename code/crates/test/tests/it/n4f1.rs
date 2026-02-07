use std::time::Duration;

use crate::{TestBuilder, TestParams};

#[tokio::test]
pub async fn proposer_fails_to_start() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1 (proposer) never starts
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

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_2() {
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
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 4 crashes at height 2
    test.add_node()
        .with_voting_power(1)
        .start()
        .wait_until(2)
        .crash()
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn two_nodes_different_voting_power() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Two nodes with high voting power
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();

    // One node with moderate voting power
    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Faulty node never starts
    test.add_node().with_voting_power(1).success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn faulty_node_recovers() {
    const HEIGHT: u64 = 8;

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
        .with_voting_power(5)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 4 crashes at height 2, restarts after 2s, and syncs up
    test.add_node()
        .with_voting_power(1)
        .start()
        .wait_until(2)
        .crash()
        .restart_after(Duration::from_secs(2))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(45),
            TestParams {
                enable_value_sync: true,
                ..Default::default()
            },
        )
        .await
}

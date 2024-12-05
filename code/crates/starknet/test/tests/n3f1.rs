use std::time::Duration;

use malachite_starknet_test::{init_logging, TestBuilder};

#[tokio::test]
pub async fn proposer_fails_to_start() {
    init_logging(module_path!());

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
    init_logging(module_path!());

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
    init_logging(module_path!());

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
    init_logging(module_path!());

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

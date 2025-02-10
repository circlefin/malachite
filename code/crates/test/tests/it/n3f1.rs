use std::time::Duration;

use crate::TestBuilder;

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

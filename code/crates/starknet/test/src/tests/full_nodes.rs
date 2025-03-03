use std::time::Duration;

use malachitebft_test_framework::TestParams;

use crate::TestBuilder;

#[tokio::test]
pub async fn basic_full_node() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Add 3 validators with different voting powers
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
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Add 2 full nodes that should follow consensus but not participate
    test.add_node()
        .full_node()
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .full_node()
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn full_node_crash_and_sync() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Add validators
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Add a full node that crashes and needs to sync
    test.add_node()
        .full_node()
        .start()
        .wait_until(3)
        .crash()
        .reset_db()
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
pub async fn late_starting_full_node() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Add validators that start immediately
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Add a full node that starts late
    test.add_node()
        .full_node()
        .start_after(1, Duration::from_secs(10))
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
pub async fn mixed_validator_and_full_node_failures() {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Add stable validators
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Add a validator that crashes
    test.add_node()
        .with_voting_power(20)
        .start()
        .wait_until(5)
        .crash()
        .restart_after(Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    // Add full nodes - one stable, one that crashes
    test.add_node()
        .full_node()
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .full_node()
        .start()
        .wait_until(6)
        .crash()
        .restart_after(Duration::from_secs(15))
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

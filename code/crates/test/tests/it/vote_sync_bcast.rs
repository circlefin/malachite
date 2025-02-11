use std::time::Duration;

use malachitebft_test_framework::{init_logging, TestBuilder, TestParams};

// NOTE: These tests are similar to the vote sync tests, with the difference that
//       the sync actor is disabled.
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
        .expect_vote_rebroadcast(2)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_custom_config(
            Duration::from_secs(60),
            TestParams {
                enable_sync: false,
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
        .expect_vote_rebroadcast(1)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_custom_config(
            Duration::from_secs(60),
            TestParams {
                enable_sync: false,
                ..Default::default()
            },
        )
        .await
}

use std::time::Duration;

use malachite_starknet_test::{Test, TestNode, TestParams};

#[tokio::test]
pub async fn two_thirds_crash() {
    const HEIGHT: u64 = 10;

    let n1 = TestNode::new(1).vp(10).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).vp(10).start().wait_until(HEIGHT).success();
    let n3 = TestNode::new(3)
        .vp(40)
        .start()
        .wait_until(2)
        .crash_after(Duration::from_millis(2000))
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    Test::new([n1, n2, n3])
        .run_with_custom_config(
            Duration::from_secs(60),
            TestParams {
                enable_blocksync: false,
                ..Default::default()
            },
        )
        .await
}

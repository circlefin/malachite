use std::time::Duration;

use malachite_starknet_test::{Test, TestNode};

#[tokio::test]
pub async fn proposer_fails_to_start() {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).vp(1).success();
    let n2 = TestNode::new(2).vp(5).start().wait_until(HEIGHT).success();
    let n3 = TestNode::new(3).vp(5).start().wait_until(HEIGHT).success();

    Test::new([n1, n2, n3]).run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn one_node_fails_to_start() {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).vp(5).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).vp(5).start().wait_until(HEIGHT).success();
    let n3 = TestNode::new(3).vp(1).success();

    Test::new([n1, n2, n3]).run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn proposer_crashes_at_height_2() {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).vp(5).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2)
        .vp(1)
        .start()
        .wait_until(2)
        .crash()
        .success();
    let n3 = TestNode::new(3).vp(5).start().wait_until(HEIGHT).success();

    Test::new([n1, n2, n3]).run(Duration::from_secs(30)).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_3() {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).vp(5).start().wait_until(HEIGHT).success();
    let n3 = TestNode::new(3).vp(5).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2)
        .vp(1)
        .start()
        .wait_until(3)
        .crash()
        .success();

    Test::new([n1, n2, n3]).run(Duration::from_secs(30)).await
}

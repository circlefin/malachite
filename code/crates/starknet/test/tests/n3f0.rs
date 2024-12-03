use std::time::Duration;

use malachite_starknet_test::{Test, TestNode};

#[tokio::test]
pub async fn all_correct_nodes() {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).start().wait_until(HEIGHT).success();
    let n3 = TestNode::new(3).start().wait_until(HEIGHT).success();

    Test::new([n1, n2, n3]).run(Duration::from_secs(30)).await
}

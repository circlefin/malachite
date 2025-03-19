use std::time::Duration;

use crate::TestBuilder;

#[tokio::test]
pub async fn all_correct_nodes() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();

    test.build().run(Duration::from_secs(30)).await
}

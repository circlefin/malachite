#![allow(unused_crate_dependencies)]

#[path = "util.rs"]
mod util;
use util::*;

#[tokio::test]
pub async fn decide_on_value() {
    let nodes = Test::new(
        [
            TestNode::correct(5),
            TestNode::correct(15),
            TestNode::correct(10),
        ],
        9,
    );

    run_test(nodes).await
}

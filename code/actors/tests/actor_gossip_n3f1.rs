#![allow(unused_crate_dependencies)]

#[path = "util.rs"]
mod util;
use util::*;

#[tokio::test]
pub async fn one_1_node_fails_to_start() {
    let nodes = Test::new(
        [
            TestNode::correct(15),
            TestNode::faulty(5, vec![Fault::NoStart]),
            TestNode::correct(10),
        ],
        6,
    );

    run_test(nodes).await
}

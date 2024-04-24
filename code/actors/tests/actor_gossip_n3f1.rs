#![allow(unused_crate_dependencies)]

#[path = "util.rs"]
mod util;
use util::*;

#[tokio::test]
pub async fn one_node_fails_to_start() {
    let nodes = Test::new(
        [
            TestNode::faulty(5, vec![Fault::NoStart]),
            TestNode::correct(15),
            TestNode::correct(10),
        ],
        4,
    );

    run_test(nodes).await
}

#[tokio::test]
pub async fn one_node_crashes() {
    let nodes = Test::new(
        [
            TestNode::faulty(5, vec![Fault::Crash(2)]),
            TestNode::correct(15),
            TestNode::correct(10),
        ],
        7,
    );

    run_test(nodes).await
}

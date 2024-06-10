#![allow(unused_crate_dependencies)]

#[path = "util.rs"]
mod util;
use util::*;

#[tokio::test]
pub async fn proposer_fails_to_start() {
    let nodes = Test::new(
        [
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        0,
    );

    run_test(nodes).await
}

#[tokio::test]
pub async fn one_node_fails_to_start() {
    let nodes = Test::new(
        [
            TestNode::correct(10),
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
        ],
        0,
    );

    run_test(nodes).await
}

#[tokio::test]
pub async fn proposer_crashes_at_height_1() {
    let nodes = Test::new(
        [
            TestNode::faulty(10, vec![Fault::Crash(1)]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        2,
    );

    run_test(nodes).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_2() {
    let nodes = Test::new(
        [
            TestNode::faulty(10, vec![Fault::Crash(2)]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        3,
    );

    run_test(nodes).await
}

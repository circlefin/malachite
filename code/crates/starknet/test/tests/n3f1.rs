#![allow(unused_crate_dependencies)]

use malachite_starknet_test::{App, Expected, Fault, Test, TestNode};

#[tokio::test]
pub async fn proposer_fails_to_start() {
    let test = Test::new(
        [
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        Expected::Exactly(0),
    );

    test.run(App::Starknet).await
}

#[tokio::test]
pub async fn one_node_fails_to_start() {
    let test = Test::new(
        [
            TestNode::correct(10),
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
        ],
        Expected::Exactly(0),
    );

    test.run(App::Starknet).await
}

#[tokio::test]
pub async fn proposer_crashes_at_height_1() {
    let test = Test::new(
        [
            TestNode::faulty(10, vec![Fault::Crash(1)]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        Expected::AtMost(4),
    );

    test.run(App::Starknet).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_2() {
    let test = Test::new(
        [
            TestNode::correct(10),
            TestNode::correct(10),
            TestNode::faulty(5, vec![Fault::Crash(2)]),
        ],
        Expected::AtMost(7),
    );

    test.run(App::Starknet).await
}

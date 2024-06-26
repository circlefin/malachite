use malachite_node::config::App;
use malachite_test::utils::test::{Fault, Test, TestNode};
use malachite_test_app::spawn::SpawnTestNode;

#[tokio::test]
pub async fn proposer_fails_to_start() {
    let test = Test::new(
        [
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        0,
    );

    test.run::<SpawnTestNode>(App::Test).await
}

#[tokio::test]
pub async fn one_node_fails_to_start() {
    let test = Test::new(
        [
            TestNode::correct(10),
            TestNode::faulty(10, vec![Fault::NoStart]),
            TestNode::correct(10),
        ],
        0,
    );

    test.run::<SpawnTestNode>(App::Test).await
}

#[tokio::test]
pub async fn proposer_crashes_at_height_1() {
    let test = Test::new(
        [
            TestNode::faulty(10, vec![Fault::Crash(1)]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        2,
    );

    test.run::<SpawnTestNode>(App::Test).await
}

#[tokio::test]
pub async fn one_node_crashes_at_height_2() {
    let test = Test::new(
        [
            TestNode::faulty(10, vec![Fault::Crash(2)]),
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        3,
    );

    test.run::<SpawnTestNode>(App::Test).await
}

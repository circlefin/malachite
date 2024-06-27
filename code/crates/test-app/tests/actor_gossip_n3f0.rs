use malachite_node::config::App;
use malachite_test::utils::test::{Test, TestNode};
use malachite_test_app::spawn::SpawnTestNode;

#[tokio::test]
pub async fn all_correct_nodes() {
    let test = Test::new(
        [
            TestNode::correct(5),
            TestNode::correct(15),
            TestNode::correct(10),
        ],
        9,
    );

    test.run::<SpawnTestNode>(App::Test).await
}

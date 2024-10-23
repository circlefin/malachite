#![allow(unused_crate_dependencies)]

use std::time::Duration;

use malachite_starknet_test::{App, Expected, Test, TestNode};

#[tokio::test]
pub async fn all_correct_nodes() {
    let test = Test::new(
        [
            TestNode::correct(5),
            TestNode::correct(15),
            TestNode::correct(10),
        ],
        Expected::AtLeast(9),
    );

    test.run(App::Starknet, Duration::from_secs(60)).await
}

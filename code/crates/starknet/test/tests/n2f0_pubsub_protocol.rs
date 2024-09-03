#![allow(unused_crate_dependencies)]

use malachite_node::config::PubSubProtocol;
use malachite_starknet_test::{App, Expected, Test, TestNode, TestParams};

async fn run_n2f0_tests(test_params: TestParams) {
    let test = Test::new(
        [TestNode::correct(10), TestNode::correct(10)],
        Expected::Exactly(6),
    );

    test.run_with_custom_config(App::Starknet, test_params)
        .await
}

#[tokio::test]
pub async fn ok_flood_all_correct_nodes_custom_config() {
    let test_params = TestParams::new(PubSubProtocol::FloodSub, 1024, 1024);
    run_n2f0_tests(test_params).await
}

// This fails due to message length limit in floodsub
#[tokio::test]
pub async fn fail_flood_all_correct_nodes_custom_config() {
    let test_params = TestParams::new(PubSubProtocol::FloodSub, 2048, 2048);
    run_n2f0_tests(test_params).await
}

#[tokio::test]
pub async fn ok_gossip_all_correct_nodes_custom_config() {
    let test_params = TestParams::new(PubSubProtocol::GossipSub, 2048, 2048);
    run_n2f0_tests(test_params).await
}


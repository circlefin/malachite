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
pub async fn flood_default_config() {
    let test = Test::new(
        [
            TestNode::correct(10),
            TestNode::correct(10),
        ],
        Expected::Exactly(6),
    );

    test.run(App::Starknet).await
}

#[tokio::test]
pub async fn flood_custom_config_1ktx() {
    let test_params = TestParams::new(PubSubProtocol::FloodSub, 1024, 1024);
    run_n2f0_tests(test_params).await
}

// This fails due to message length limit in floodsub
#[tokio::test]
pub async fn flood_custom_config_2ktx() {
    let test_params = TestParams::new(PubSubProtocol::FloodSub, 2048, 2048);
    run_n2f0_tests(test_params).await
}

#[tokio::test]
pub async fn gossip_custom_config_2ktx() {
    let test_params = TestParams::new(PubSubProtocol::GossipSub, 2048, 2048);
    run_n2f0_tests(test_params).await
}


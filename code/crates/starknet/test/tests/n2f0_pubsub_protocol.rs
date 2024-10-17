#![allow(unused_crate_dependencies)]

use bytesize::ByteSize;
use malachite_node::config::{GossipSubConfig, PubSubProtocol};
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
        [TestNode::correct(10), TestNode::correct(10)],
        Expected::Exactly(6),
    );

    test.run(App::Starknet).await
}

#[tokio::test]
pub async fn broadcast_custom_config_1ktx() {
    let test_params = TestParams::new(
        PubSubProtocol::Broadcast,
        ByteSize::kib(1),
        ByteSize::kib(1),
    );

    run_n2f0_tests(test_params).await
}

#[tokio::test]
pub async fn broadcast_custom_config_2ktx() {
    let test_params = TestParams::new(
        PubSubProtocol::Broadcast,
        ByteSize::kib(2),
        ByteSize::kib(2),
    );

    run_n2f0_tests(test_params).await
}

#[tokio::test]
pub async fn gossip_custom_config_1ktx() {
    let test_params = TestParams::new(
        PubSubProtocol::GossipSub(GossipSubConfig::default()),
        ByteSize::kib(1),
        ByteSize::kib(1),
    );
    run_n2f0_tests(test_params).await
}

#[tokio::test]
pub async fn gossip_custom_config_2ktx() {
    let test_params = TestParams::new(
        PubSubProtocol::GossipSub(GossipSubConfig::default()),
        ByteSize::kib(2),
        ByteSize::kib(2),
    );

    run_n2f0_tests(test_params).await
}

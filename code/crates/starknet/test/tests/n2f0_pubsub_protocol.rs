#![allow(unused_crate_dependencies)]

use std::time::Duration;

use bytesize::ByteSize;
use malachite_config::{GossipSubConfig, PubSubProtocol};
use malachite_starknet_test::{App, Expected, Test, TestNode, TestParams};

async fn run_n2f0_tests(params: TestParams) {
    let test = Test::new(
        [TestNode::correct(10), TestNode::correct(10)],
        Expected::Exactly(6),
    );

    test.run_with_custom_config(App::Starknet, Duration::from_secs(30), params)
        .await
}

#[tokio::test]
pub async fn broadcast_custom_config_1ktx() {
    let params = TestParams {
        enable_blocksync: false,
        protocol: PubSubProtocol::Broadcast,
        block_size: ByteSize::kib(1),
        tx_size: ByteSize::kib(1),
        txs_per_part: 1,
    };

    run_n2f0_tests(params).await
}

#[tokio::test]
pub async fn broadcast_custom_config_2ktx() {
    let params = TestParams {
        enable_blocksync: false,
        protocol: PubSubProtocol::Broadcast,
        block_size: ByteSize::kib(2),
        tx_size: ByteSize::kib(2),
        txs_per_part: 1,
    };

    run_n2f0_tests(params).await
}

#[tokio::test]
pub async fn gossip_custom_config_1ktx() {
    let params = TestParams {
        enable_blocksync: false,
        protocol: PubSubProtocol::GossipSub(GossipSubConfig::default()),
        block_size: ByteSize::kib(1),
        tx_size: ByteSize::kib(1),
        txs_per_part: 1,
    };

    run_n2f0_tests(params).await
}

#[tokio::test]
pub async fn gossip_custom_config_2ktx() {
    let params = TestParams {
        enable_blocksync: false,
        protocol: PubSubProtocol::GossipSub(GossipSubConfig::default()),
        block_size: ByteSize::kib(2),
        tx_size: ByteSize::kib(2),
        txs_per_part: 1,
    };

    run_n2f0_tests(params).await
}

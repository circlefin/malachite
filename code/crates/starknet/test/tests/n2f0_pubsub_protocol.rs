use std::time::Duration;

use bytesize::ByteSize;
use malachite_config::{GossipSubConfig, PubSubProtocol};
use malachite_starknet_test::{Test, TestNode, TestParams};

async fn run_n2f0_tests(params: TestParams) {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).start().wait_until(HEIGHT).success();

    Test::new([n1, n2])
        .run_with_custom_config(Duration::from_secs(30), params)
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
    };

    run_n2f0_tests(params).await
}

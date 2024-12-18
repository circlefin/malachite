use std::time::Duration;

use bytesize::ByteSize;
use informalsystems_malachitebft_starknet_test::{init_logging, TestBuilder, TestParams};
use malachitebft_config::{GossipSubConfig, PubSubProtocol};

async fn run_test(params: TestParams) {
    init_logging(module_path!());

    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();

    test.build()
        .run_with_custom_config(Duration::from_secs(30), params)
        .await
}

#[tokio::test]
pub async fn broadcast_custom_config_1ktx() {
    let params = TestParams {
        enable_sync: false,
        protocol: PubSubProtocol::Broadcast,
        block_size: ByteSize::kib(1),
        tx_size: ByteSize::kib(1),
        txs_per_part: 1,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
pub async fn broadcast_custom_config_2ktx() {
    let params = TestParams {
        enable_sync: false,
        protocol: PubSubProtocol::Broadcast,
        block_size: ByteSize::kib(2),
        tx_size: ByteSize::kib(2),
        txs_per_part: 1,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
pub async fn gossip_custom_config_1ktx() {
    let params = TestParams {
        enable_sync: false,
        protocol: PubSubProtocol::GossipSub(GossipSubConfig::default()),
        block_size: ByteSize::kib(1),
        tx_size: ByteSize::kib(1),
        txs_per_part: 1,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
pub async fn gossip_custom_config_2ktx() {
    let params = TestParams {
        enable_sync: false,
        protocol: PubSubProtocol::GossipSub(GossipSubConfig::default()),
        block_size: ByteSize::kib(2),
        tx_size: ByteSize::kib(2),
        txs_per_part: 1,
        ..Default::default()
    };

    run_test(params).await
}

use std::time::Duration;

use informalsystems_malachitebft_starknet_test::{init_logging, TestBuilder, TestParams};
use malachitebft_config::ValuePayload;

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
pub async fn parts_only() {
    let params = TestParams {
        value_payload: ValuePayload::PartsOnly,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
pub async fn proposal_only() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
pub async fn proposal_and_parts() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..Default::default()
    };

    run_test(params).await
}

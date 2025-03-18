use std::time::Duration;

use malachitebft_config::ValuePayload;

use crate::{TestBuilder, TestParams};

async fn run_test(params: TestParams) {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();

    test.build()
        .run_with_params(Duration::from_secs(30), params)
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
#[ignore] // Test app only supports parts-only mode
pub async fn proposal_and_parts() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..Default::default()
    };

    run_test(params).await
}

#[tokio::test]
#[ignore] // This functionality is not fully implemented yet
pub async fn proposal_only() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..Default::default()
    };

    run_test(params).await
}

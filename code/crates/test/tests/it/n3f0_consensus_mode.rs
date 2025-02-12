use std::time::Duration;

use malachitebft_config::ValuePayload;
use malachitebft_test_framework::{init_logging, TestBuilder, TestParams};

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
pub async fn proposal_and_parts() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..Default::default()
    };

    run_test(params).await
}

// This functionality is not fully implemented yet
#[tokio::test]
#[ignore]
pub async fn proposal_only() {
    let params = TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..Default::default()
    };

    run_test(params).await
}

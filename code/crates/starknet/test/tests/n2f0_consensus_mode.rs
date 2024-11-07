#![allow(unused_crate_dependencies)]

use std::time::Duration;

use malachite_config::ValuePayload;
use malachite_starknet_test::{Test, TestNode, TestParams};

async fn run_test(params: TestParams) {
    const HEIGHT: u64 = 5;

    let n1 = TestNode::new(1).start().wait_until(HEIGHT).success();
    let n2 = TestNode::new(2).start().wait_until(HEIGHT).success();

    Test::new([n1, n2])
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

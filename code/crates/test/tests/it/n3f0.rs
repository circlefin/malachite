use std::time::Duration;

use malachitebft_test_framework::TestParams;

use malachitebft_config::{ValuePayload, VoteSyncMode};

use crate::TestBuilder;

#[tokio::test]
pub async fn all_correct_nodes() {
    const HEIGHT: u64 = 2;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();
    test.add_node().start().wait_until(HEIGHT).success();

    test.build()
        .run_with_params(
            Duration::from_secs(50),
            TestParams {
                vote_sync_mode: Some(VoteSyncMode::RequestResponse),
                value_payload: ValuePayload::ProposalAndParts,
                ..TestParams::default()
            },
        )
        .await
}

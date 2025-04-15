use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use eyre::bail;
use informalsystems_malachitebft_test::middleware::Middleware;
use informalsystems_malachitebft_test::TestContext;
use malachitebft_config::VoteSyncMode;
use malachitebft_core_consensus::ProposedValue;
use malachitebft_core_types::CommitCertificate;
use malachitebft_test_framework::TestParams;

use crate::TestBuilder;

#[tokio::test]
pub async fn reset_height() {
    const RESET_HEIGHT: u64 = 5;
    const FINAL_HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node().start().wait_until(FINAL_HEIGHT).success();
    test.add_node().start().wait_until(FINAL_HEIGHT).success();

    test.add_node()
        .with_middleware(ResetHeight::new(RESET_HEIGHT))
        .start()
        .wait_until(RESET_HEIGHT) // First time reaching height
        .wait_until(RESET_HEIGHT) // Will restart height after commit failure
        .wait_until(FINAL_HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                vote_sync_mode: Some(VoteSyncMode::RequestResponse),
                ..TestParams::default()
            },
        )
        .await
}

#[derive(Debug)]
struct ResetHeight {
    reset_height: u64,
    reset: AtomicBool,
}

impl ResetHeight {
    fn new(reset_height: u64) -> Self {
        Self {
            reset_height,
            reset: AtomicBool::new(false),
        }
    }
}

impl Middleware for ResetHeight {
    fn on_commit(
        &self,
        _ctx: &TestContext,
        certificate: &CommitCertificate<TestContext>,
        proposal: &ProposedValue<TestContext>,
    ) -> Result<(), eyre::Report> {
        assert_eq!(certificate.height, proposal.height);

        if certificate.height.as_u64() == self.reset_height
            && !self.reset.swap(true, Ordering::SeqCst)
        {
            bail!("Simulating commit failure");
        }

        Ok(())
    }
}

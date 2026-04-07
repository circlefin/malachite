use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytesize::ByteSize;
use eyre::bail;
use rstest::rstest;

use arc_malachitebft_test::middleware::{Middleware, RotateEpochValidators};
use arc_malachitebft_test::TestContext;
use malachitebft_config::ValuePayload;
use malachitebft_core_consensus::ProposedValue;
use malachitebft_core_types::CommitCertificate;

use crate::{TestBuilder, TestParams};

pub async fn crash_restart_from_start(params: TestParams) {
    const HEIGHT: u64 = 6;
    const CRASH_HEIGHT: u64 = 4;

    let mut test = TestBuilder::<()>::new();

    // Node 1 starts with 10 voting power.
    test.add_node()
        .with_voting_power(10)
        .start()
        // Wait until it reaches height 10
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    // Node 2 starts with 10 voting power, in parallel with node 1 and with the same behaviour
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3 starts with 5 voting power, in parallel with node 1 and 2.
    test.add_node()
        .with_voting_power(5)
        .start()
        // Wait until the node reaches height 2...
        .wait_until(CRASH_HEIGHT)
        // ...and then kills it
        .crash()
        // Reset the database so that the node has to do Sync from height 1
        .reset_db()
        // After that, it waits 5 seconds before restarting the node
        .restart_after(Duration::from_secs(5))
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                target_time: Some(Duration::from_millis(15)),
                enable_value_sync: true, // Enable Sync
                ..params
            },
        )
        .await
}

#[rstest]
#[case::parts_only_eager(ValuePayload::PartsOnly, Duration::ZERO)]
#[case::parts_only_interval(ValuePayload::PartsOnly, Duration::from_secs(1))]
#[case::proposal_and_parts_eager(ValuePayload::ProposalAndParts, Duration::ZERO)]
#[case::proposal_and_parts_interval(ValuePayload::ProposalAndParts, Duration::from_secs(1))]
#[tokio::test]
pub async fn crash_restart_from_start_ok(
    #[case] value_payload: ValuePayload,
    #[case] status_update_interval: Duration,
) {
    crash_restart_from_start(TestParams {
        value_payload,
        status_update_interval,
        ..Default::default()
    })
    .await
}

#[rstest]
#[case::proposal_only_eager(Duration::ZERO)]
#[case::proposal_only_interval(Duration::from_secs(1))]
#[tokio::test]
#[ignore]
pub async fn crash_restart_from_start_proposal_only(#[case] status_update_interval: Duration) {
    crash_restart_from_start(TestParams {
        value_payload: ValuePayload::ProposalOnly,
        status_update_interval,
        ..Default::default()
    })
    .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn crash_restart_from_latest(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(2)
        .crash()
        // We do not reset the database so that the node can restart from the latest height
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn aggressive_pruning(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 15;

    let mut test = TestBuilder::<()>::new();

    // Node 1 starts with 10 voting power.
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start()
        .wait_until(2)
        .crash()
        .reset_db()
        .restart_after(Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60), // Timeout for the whole test
            TestParams {
                enable_value_sync: true, // Enable Sync
                max_retain_blocks: 10,   // Prune blocks older than 10
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn start_late(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn start_late_parallel_requests(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(5)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 5,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn start_late_parallel_requests_with_batching(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(0)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 2,
                batch_size: 2,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn start_late_rotate_epoch_validator_set(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 20;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(RotateEpochValidators {
            selection_size: 2,
            epochs_limit: 5,
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(RotateEpochValidators {
            selection_size: 2,
            epochs_limit: 5,
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(RotateEpochValidators {
            selection_size: 2,
            epochs_limit: 5,
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    // Add 2 full nodes with one starting late
    test.add_node()
        .full_node()
        .with_middleware(RotateEpochValidators {
            selection_size: 2,
            epochs_limit: 5,
        })
        .start()
        .wait_until(HEIGHT)
        .success();
    test.add_node()
        .full_node()
        .with_middleware(RotateEpochValidators {
            selection_size: 2,
            epochs_limit: 5,
        })
        .start_after(1, Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn sync_only_fullnode_without_consensus(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 8;

    let mut test = TestBuilder::<()>::new();

    // First two nodes are normal validators that will drive consensus
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Third node is a sync-only full node (0 voting power, consensus disabled)
    // It should be able to sync values but not participate in consensus
    test.add_node()
        .full_node()
        .disable_consensus()
        .start_after(1, Duration::from_secs(5)) // Start late to force syncing
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(45),
            // NOTE: consensus is enabled by default for other nodes
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                status_update_interval,
                ..Default::default()
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

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn reset_height(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 10;
    const RESET_HEIGHT: u64 = 1;
    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(0)
        .with_middleware(ResetHeight::new(RESET_HEIGHT))
        .start_after(1, Duration::from_secs(10))
        .wait_until(RESET_HEIGHT) // First time reaching height
        .wait_until(RESET_HEIGHT)
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                batch_size: 2,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn full_node_sync_after_all_persistent_peer_restart(
    #[case] status_update_interval: Duration,
) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1-3: validators that will restart
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(4))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(4))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(4))
        .wait_until(HEIGHT + 5)
        .success();

    // Node 4: full node that syncs and should resume syncing all validators have restarted
    test.add_node()
        .full_node()
        .start_after(1, Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(30),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn validator_persistent_peer_reconnection_discovery_enabled(
    #[case] status_update_interval: Duration,
) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1: validator that stays up initially
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        // Stop this node to simulate network partition
        .crash()
        // Wait before restarting to test reconnection
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5) // Continue after restart
        .success();

    // Node 2: validator that stays up initially
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        // Stop this node to simulate network partition
        .crash()
        // Wait before restarting to test reconnection
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5) // Continue after restart
        .success();

    // Node 3: validator that stays up initially
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        // Stop this node to simulate network partition
        .crash()
        // Wait before restarting to test reconnection
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5) // Continue after restart
        .success();

    // Node 4: validator that that syncs and needs to reconnect after all validators have restarted
    test.add_node()
        .with_voting_power(5)
        .start_after(1, Duration::from_secs(12))
        // This node should reconnect to peers when they restart and continue syncing
        .wait_until(HEIGHT + 5)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                enable_discovery: true,
                exclude_from_persistent_peers: vec![4], // Node 4 is a new validator, others don't have it as persistent peer
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn validator_persistent_peer_reconnection_discovery_disabled(
    #[case] status_update_interval: Duration,
) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1-3: validators that will restart
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    // Node 4: validator that that syncs and needs to reconnect after all validators have restarted
    test.add_node()
        .with_voting_power(5)
        .start_after(1, Duration::from_secs(12))
        .wait_until(HEIGHT + 5)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 1,
                enable_discovery: false,
                exclude_from_persistent_peers: vec![4], // Node 4 is a new validator, others don't have it as persistent peer
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn full_node_persistent_peer_reconnection_discovery_enabled(
    #[case] status_update_interval: Duration,
) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1-3: validators that will restart
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    // Node 4: full node that that syncs and needs to reconnect after all validators have restarted
    test.add_node()
        .full_node()
        .start_after(1, Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                enable_discovery: true,
                // Node 4 is a full node, other validators don't have it as persistent peer
                exclude_from_persistent_peers: vec![4],
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn full_node_persistent_peer_reconnection_discovery_disabled(
    #[case] status_update_interval: Duration,
) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    // Node 1-3: validators that will restart
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    // Node 4: full node that syncs and needs to reconnect after all validators have restarted
    test.add_node()
        .full_node()
        .start_after(1, Duration::from_secs(3))
        .wait_until(HEIGHT + 5)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                parallel_requests: 3,
                enable_discovery: false,
                // Node 4 is a full node, other validators don't have it as persistent peer
                exclude_from_persistent_peers: vec![4],
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn response_size_limit_exceeded(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3 starts with 5 voting power, in parallel with node 1 and 2.
    test.add_node()
        .with_voting_power(5)
        .start()
        // Wait until the node reaches height 2...
        .wait_until(2)
        // ...and then kills it
        .crash()
        // Reset the database so that the node has to do Sync from height 1
        .reset_db()
        // After that, it waits 5 seconds before restarting the node
        .restart_after(Duration::from_secs(5))
        // Wait until the node reached the expected height
        .wait_until(HEIGHT)
        // Record a successful test for this node
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                // Values are around ~900 bytes, so this `max_response_size` in combination
                // with a `batch_size` of 2 leads to having a syncing peer sending partial responses.
                max_response_size: ByteSize::b(1000),
                // Values are around ~900 bytes, so we cannot have more than one value in a response.
                // In other words, if `max_response_size` is not respected, node 3 would not have been
                // able to sync in this test.
                rpc_max_size: ByteSize::b(1000),
                batch_size: 2,
                parallel_requests: 1,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn status_update_on_decision(#[case] status_update_interval: Duration) {
    const HEIGHT: u64 = 10;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(0)
        .start_after(1, Duration::from_secs(10))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

/// Middleware that skips early decision commit in the `AppMsg::Decided` handler
/// for a range of heights. The decision is still committed during `AppMsg::Finalized`.
#[derive(Debug)]
struct SkipEarlyCommitMiddleware {
    from_height: u64,
    to_height: u64,
}

impl Middleware for SkipEarlyCommitMiddleware {
    fn skip_early_commit(
        &self,
        _ctx: &TestContext,
        certificate: &CommitCertificate<TestContext>,
    ) -> bool {
        let h = certificate.height.as_u64();
        h >= self.from_height && h <= self.to_height
    }
}

/// A full node should be able to sync from validators even when the `Decided` handler
/// does not commit decisions (they are only committed during `Finalized`).
///
/// The sync actor must not advertise a height until the decision is confirmed committed,
/// so peers always receive complete responses.
#[rstest]
#[case::eager(Duration::ZERO)]
#[case::interval(Duration::from_secs(1))]
#[tokio::test]
pub async fn skipped_early_commit_does_not_break_sync(#[case] status_update_interval: Duration) {
    const TARGET_HEIGHT: u64 = 6;
    const SKIP_FROM: u64 = 1;
    const SKIP_TO: u64 = 100;

    let mut test = TestBuilder::<()>::new();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(SkipEarlyCommitMiddleware {
            from_height: SKIP_FROM,
            to_height: SKIP_TO,
        })
        .start()
        .wait_until(TARGET_HEIGHT * 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(SkipEarlyCommitMiddleware {
            from_height: SKIP_FROM,
            to_height: SKIP_TO,
        })
        .start()
        .wait_until(TARGET_HEIGHT * 2)
        .success();

    // Full node starts late, must sync from the validators.
    test.add_node()
        .full_node()
        .start_after(1, Duration::from_secs(10))
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                status_update_interval,
                ..Default::default()
            },
        )
        .await
}

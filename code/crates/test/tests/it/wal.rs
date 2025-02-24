use std::time::Duration;

use eyre::bail;
use tracing::info;

use informalsystems_malachitebft_test as malachitebft_test;

use malachitebft_config::ValuePayload;
use malachitebft_core_consensus::LocallyProposedValue;
use malachitebft_core_types::SignedVote;
use malachitebft_engine::util::events::Event;
use malachitebft_test::TestContext;

use crate::{HandlerResult, TestBuilder, TestParams};

#[tokio::test]
async fn proposer_crashes_after_proposing_parts_only() {
    proposer_crashes_after_proposing(TestParams {
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore] // Test app only supports parts-only mode
async fn proposer_crashes_after_proposing_proposal_and_parts() {
    proposer_crashes_after_proposing(TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore] // Test app only supports parts-only mode
async fn proposer_crashes_after_proposing_proposal_only() {
    proposer_crashes_after_proposing(TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..TestParams::default()
    })
    .await
}

async fn proposer_crashes_after_proposing(params: TestParams) {
    #[derive(Clone, Debug, Default)]
    struct State {
        first_proposed_value: Option<LocallyProposedValue<TestContext>>,
    }

    const CRASH_HEIGHT: u64 = 3;

    let mut test = TestBuilder::<State>::new();

    test.add_node().with_voting_power(10).start().success();
    test.add_node().with_voting_power(10).start().success();

    test.add_node()
        .with_voting_power(40)
        .start()
        .wait_until(CRASH_HEIGHT)
        // Wait until this node proposes a value
        .on_event(|event, state| match event {
            Event::ProposedValue(value) => {
                info!("Proposer proposed block: {:?}", value.value);
                state.first_proposed_value = Some(value);
                Ok(HandlerResult::ContinueTest)
            }
            _ => Ok(HandlerResult::WaitForNextEvent),
        })
        // Crash right after
        .crash()
        // Restart after 5 seconds
        .restart_after(Duration::from_secs(5))
        // Check that we replay messages from the WAL
        .expect_wal_replay(CRASH_HEIGHT)
        // Wait until it proposes a value again, while replaying WAL
        // Check that it is the same value as the first time
        .on_proposed_value(|value, state| {
            let Some(first_value) = state.first_proposed_value.as_ref() else {
                bail!("Proposer did not propose a block");
            };

            if first_value.value == value.value {
                info!("Proposer re-proposed the same block: {:?}", value.value);
                Ok(HandlerResult::ContinueTest)
            } else {
                bail!(
                    "Proposer just equivocated: expected {:?}, got {:?}",
                    first_value.value,
                    value.value
                )
            }
        })
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: false,
                ..params
            },
        )
        .await
}

#[tokio::test]
async fn non_proposer_crashes_after_voting_parts_only() {
    non_proposer_crashes_after_voting(TestParams {
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore] // Test app only supports parts-only mode
async fn non_proposer_crashes_after_voting_proposal_and_parts() {
    non_proposer_crashes_after_voting(TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore] // Test app only supports parts-only mode
async fn non_proposer_crashes_after_voting_proposal_only() {
    non_proposer_crashes_after_voting(TestParams {
        value_payload: ValuePayload::ProposalOnly,
        ..TestParams::default()
    })
    .await
}

async fn non_proposer_crashes_after_voting(params: TestParams) {
    #[derive(Clone, Debug, Default)]
    struct State {
        first_vote: Option<SignedVote<TestContext>>,
    }

    const CRASH_HEIGHT: u64 = 4;

    let mut test = TestBuilder::<State>::new();

    test.add_node()
        .with_voting_power(40)
        .start()
        .wait_until(CRASH_HEIGHT)
        // Wait until this node proposes a value
        .on_vote(|vote, state| {
            info!("Non-proposer voted");
            state.first_vote = Some(vote);

            Ok(HandlerResult::ContinueTest)
        })
        // Crash right after
        .crash()
        // Restart after 5 seconds
        .restart_after(Duration::from_secs(5))
        // Check that we replay messages from the WAL
        .expect_wal_replay(CRASH_HEIGHT)
        // Wait until it proposes a value again, while replaying WAL
        // Check that it is the same value as the first time
        .on_vote(|vote, state| {
            let Some(first_vote) = state.first_vote.as_ref() else {
                bail!("Non-proposer did not vote")
            };

            if first_vote.value == vote.value {
                info!("Non-proposer voted the same way: {first_vote:?}");
                Ok(HandlerResult::ContinueTest)
            } else {
                bail!(
                    "Non-proposer just equivocated: expected {:?}, got {:?}",
                    first_vote.value,
                    vote.value
                )
            }
        })
        .success();

    test.add_node().with_voting_power(10).start().success();
    test.add_node().with_voting_power(10).start().success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: false,
                ..params
            },
        )
        .await
}

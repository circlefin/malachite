use std::time::Duration;

use eyre::bail;
use informalsystems_malachitebft_test::middleware::Middleware;
use tracing::info;

use informalsystems_malachitebft_test::{self as malachitebft_test};

use malachitebft_config::{ValuePayload, VoteSyncMode};
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
async fn proposer_crashes_after_proposing_proposal_and_parts() {
    proposer_crashes_after_proposing(TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
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
async fn non_proposer_crashes_after_voting_proposal_and_parts() {
    non_proposer_crashes_after_voting(TestParams {
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
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

    const CRASH_HEIGHT: u64 = 2;

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
        // Wait until the previous vote is replayed
        // Check that it is the for the same value as the first time
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
        .wait_until(CRASH_HEIGHT + 2)
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

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_1_request_response_parts_only() {
    byzantine_proposer_crashes_after_proposing_1(TestParams {
        vote_sync_mode: Some(VoteSyncMode::RequestResponse),
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_1_request_response_proposal_and_parts() {
    byzantine_proposer_crashes_after_proposing_1(TestParams {
        vote_sync_mode: Some(VoteSyncMode::RequestResponse),
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_1_rebroadcast_parts_only() {
    byzantine_proposer_crashes_after_proposing_1(TestParams {
        vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_1_rebroadcast_proposal_and_parts() {
    byzantine_proposer_crashes_after_proposing_1(TestParams {
        vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

async fn byzantine_proposer_crashes_after_proposing_1(params: TestParams) {
    #[derive(Clone, Debug, Default)]
    struct State {
        first_proposed_value: Option<LocallyProposedValue<TestContext>>,
    }

    const CRASH_HEIGHT: u64 = 3;

    let mut test = TestBuilder::<State>::new();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(CRASH_HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(5))
        .wait_until(CRASH_HEIGHT + 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(CRASH_HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(5))
        .wait_until(CRASH_HEIGHT + 2)
        .success();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(ByzantineProposer)
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
        .on_proposed_value(|value, state| {
            let Some(first_value) = state.first_proposed_value.as_ref() else {
                bail!("Proposer did not propose a block");
            };

            if first_value.value == value.value {
                bail!(
                    "Byzantine proposer unexpectedly re-proposed the same value: {:?}",
                    value.value
                );
            }

            info!(
                "As per the test, the proposer just equivocated: expected {:?}, got {:?}",
                first_value.value, value.value
            );

            Ok(HandlerResult::ContinueTest)
        })
        .wait_until(CRASH_HEIGHT + 2)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                ..params
            },
        )
        .await
}

#[tokio::test]
async fn restart_with_byzantine_proposer_2_request_response_parts_only() {
    byzantine_proposer_crashes_after_proposing_2(TestParams {
        vote_sync_mode: Some(VoteSyncMode::RequestResponse),
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
async fn restart_with_byzantine_proposer_2_request_response_proposal_and_parts() {
    byzantine_proposer_crashes_after_proposing_2(TestParams {
        vote_sync_mode: Some(VoteSyncMode::RequestResponse),
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_2_rebroadcast_parts_only() {
    byzantine_proposer_crashes_after_proposing_2(TestParams {
        vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
        value_payload: ValuePayload::PartsOnly,
        ..TestParams::default()
    })
    .await
}

#[tokio::test]
#[ignore]
async fn restart_with_byzantine_proposer_2_rebroadcast_proposal_and_parts() {
    byzantine_proposer_crashes_after_proposing_2(TestParams {
        vote_sync_mode: Some(VoteSyncMode::Rebroadcast),
        value_payload: ValuePayload::ProposalAndParts,
        ..TestParams::default()
    })
    .await
}

async fn byzantine_proposer_crashes_after_proposing_2(params: TestParams) {
    #[derive(Clone, Debug, Default)]
    struct State {
        first_proposed_value: Option<LocallyProposedValue<TestContext>>,
        first_vote: Option<SignedVote<TestContext>>,
    }

    const CRASH_HEIGHT: u64 = 3;

    let mut test = TestBuilder::<State>::new();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(CRASH_HEIGHT)
        .crash()
        .restart_after(Duration::from_secs(6))
        .success();

    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(CRASH_HEIGHT)
        .on_vote(|vote, state| {
            info!("Non-proposer voted");
            state.first_vote = Some(vote);

            Ok(HandlerResult::ContinueTest)
        })
        // Crash right after
        .crash()
        .restart_after(Duration::from_secs(5))
        .success();

    test.add_node()
        .with_voting_power(10)
        .with_middleware(ByzantineProposer)
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
        .on_proposed_value(|value, state| {
            let Some(first_value) = state.first_proposed_value.as_ref() else {
                bail!("Proposer did not propose a block");
            };

            if first_value.value == value.value {
                bail!(
                    "Byzantine proposer unexpectedly re-proposed the same value: {:?}",
                    value.value
                );
            }

            info!(
                "As per the test, the proposer just equivocated: expected {:?}, got {:?}",
                first_value.value, value.value
            );

            Ok(HandlerResult::ContinueTest)
        })
        .wait_until(CRASH_HEIGHT + 2)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                timeout_step: Duration::from_secs(5),
                value_payload: ValuePayload::PartsOnly,
                ..params
            },
        )
        .await
}

#[derive(Copy, Clone, Debug)]
struct ByzantineProposer;

impl Middleware for ByzantineProposer {
    fn on_propose_value(
        &self,
        _ctx: &TestContext,
        proposal: &mut LocallyProposedValue<TestContext>,
        reproposal: bool,
    ) {
        use informalsystems_malachitebft_test::Value;
        use rand::Rng;

        if !reproposal {
            tracing::warn!(
                "ByzantineProposer: First time proposing value {:}",
                proposal.value.id()
            );

            // Do not change the value if it is the first time we propose it
            return;
        }

        // Make up a new value that is different from the one we are supposed to propose
        let new_value = loop {
            let new_value = Value::new(rand::thread_rng().gen_range(100..=100000));
            if new_value != proposal.value {
                break new_value;
            }
        };

        tracing::warn!(
            "ByzantineProposer: Not re-using previously built value {:} but a new one {:}",
            proposal.value.id(),
            new_value.id()
        );

        proposal.value = new_value;
    }
}

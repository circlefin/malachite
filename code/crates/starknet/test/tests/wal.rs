use std::time::Duration;

use malachite_actors::util::events::Event;
use malachite_common::NilOrVal;
use malachite_consensus::SignedConsensusMsg;
use malachite_starknet_host::types::BlockHash;
use malachite_starknet_test::{init_logging, HandlerResult, Test, TestNode, TestParams};
use tracing::info;

#[tokio::test]
pub async fn proposer_crashes_after_proposing() {
    init_logging(module_path!());

    #[derive(Clone, Debug, Default)]
    struct State {
        block_hash: Option<BlockHash>,
    }

    let n1 = TestNode::with_state(2, State::default())
        .vp(10)
        .start()
        .success();

    let n2 = TestNode::with_state(3, State::default())
        .vp(10)
        .start()
        .success();

    let n3 = TestNode::with_state(3, State::default())
        .vp(40)
        .start()
        .wait_until(3)
        // Wait until this node proposes a value
        .on_event(|event, state| match event {
            Event::ProposedValue(value) => {
                info!("Proposer proposed block: {:?}", value.value);
                state.block_hash = Some(value.value);
                Ok(HandlerResult::ContinueTest)
            }
            _ => Ok(HandlerResult::WaitForNextEvent),
        })
        // Crash right after
        .crash()
        // Restart after 5 seconds
        .restart_after(Duration::from_secs(5))
        // Wait until it proposes a value again, while replaying WAL
        // Check that it is the same value as the first time
        .on_event(|event, state| {
            let Some(first_value) = state.block_hash.as_ref() else {
                return Err("Proposer did not propose a block".into());
            };

            if let Event::ProposedValue(value) = event {
                if first_value == &value.value {
                    info!("Proposer re-proposed the same block: {:?}", value.value);
                    Ok(HandlerResult::ContinueTest)
                } else {
                    Err(format!(
                        "Proposer just equivocated: expected {:?}, got {:?}",
                        first_value, value.value
                    )
                    .into())
                }
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
        .success();

    Test::new([n1, n2, n3])
        .run_with_custom_config(
            Duration::from_secs(30),
            TestParams {
                enable_blocksync: false,
                ..Default::default()
            },
        )
        .await
}

#[tokio::test]
pub async fn non_proposer_crashes_after_voting() {
    init_logging(module_path!());

    #[derive(Clone, Debug, Default)]
    struct State {
        voted_for: Option<NilOrVal<BlockHash>>,
    }

    let n1 = TestNode::with_state(1, State::default())
        .vp(10)
        .start()
        .success();

    let n2 = TestNode::with_state(2, State::default())
        .vp(10)
        .start()
        .success();

    let n3 = TestNode::with_state(3, State::default())
        .vp(40)
        .start()
        .wait_until(3)
        // Wait until this node proposes a value
        .on_event(|event, state| match event {
            Event::Published(SignedConsensusMsg::Vote(vote)) => {
                info!("Non-proposer voted");
                state.voted_for = Some(vote.block_hash);
                Ok(HandlerResult::ContinueTest)
            }
            _ => Ok(HandlerResult::WaitForNextEvent),
        })
        // Crash right after
        .crash()
        // Restart after 5 seconds
        .restart_after(Duration::from_secs(5))
        // Wait until it proposes a value again, while replaying WAL
        // Check that it is the same value as the first time
        .on_event(|event, state| {
            let Some(first_vote) = state.voted_for.as_ref() else {
                return Err("Non-proposer did not vote".into());
            };

            if let Event::Published(SignedConsensusMsg::Vote(second_vote)) = event {
                if first_vote == &second_vote.block_hash {
                    info!("Non-proposer voted the same way: {first_vote:?}");
                    Ok(HandlerResult::ContinueTest)
                } else {
                    Err(format!(
                        "Non-proposer just equivocated: expected {:?}, got {:?}",
                        first_vote, second_vote.block_hash
                    )
                    .into())
                }
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
        .success();

    Test::new([n1, n2, n3])
        .run_with_custom_config(
            Duration::from_secs(30),
            TestParams {
                enable_blocksync: false,
                ..Default::default()
            },
        )
        .await
}

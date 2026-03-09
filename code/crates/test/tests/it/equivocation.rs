use std::{collections::HashSet, time::Duration};

use malachitebft_core_consensus::MisbehaviorEvidence;
use malachitebft_core_types::{Context, Proposal, Vote};
use malachitebft_test_framework::{HandlerResult, TestParams};

use crate::middlewares::PrevoteRandom;
use crate::TestBuilder;

const TARGET_TIME: Duration = Duration::from_secs(1);

#[allow(clippy::never_loop)]
fn check_decided_impl<Ctx: Context>(evidence: &MisbehaviorEvidence<Ctx>) {
    for addr in evidence.proposals.iter() {
        let list = evidence.proposals.get(addr).unwrap();
        if let Some((p1, p2)) = list.first() {
            assert_ne!(p1.value(), p2.value());
        }
    }

    for addr in evidence.votes.iter() {
        let list = evidence.votes.get(addr).unwrap();
        if let Some((v1, v2)) = list.first() {
            assert_eq!(v1.round(), v2.round());
            assert_eq!(v1.vote_type(), v2.vote_type());
            assert_ne!(v1.value(), v2.value());
        }
    }
}

#[tokio::test]
pub async fn equivocation_two_vals_same_key_proposal() {
    // Nodes 1 and 2 share a validator key to induce proposal equivocation.
    let params = TestParams {
        shared_key_group: HashSet::from([1, 2]),
        target_time: Some(TARGET_TIME),
        ..Default::default()
    };
    let mut test = TestBuilder::<()>::new();

    // Node 1 - Byzantine
    test.add_node().start().success();

    // Node 2  - Byzantine: same validator key as node 1
    test.add_node().start().success();

    // Node 3: correct, with >2/3 of the total voting power.
    // Checks for proposal equivocation.
    // Voting power is set to hold >2/3 worth of VP so consensus always progresses.
    // Proposals are processed regardless of consensus state.
    test.add_node()
        .with_voting_power(5)
        .start()
        .on_finalized(|_c, evidence, _s| {
            check_decided_impl(&evidence);
            let result = if evidence.proposals.is_empty() {
                HandlerResult::WaitForNextEvent
            } else {
                HandlerResult::ContinueTest
            };
            Ok(result)
        })
        .success();

    test.build()
        .run_with_params(Duration::from_secs(15), params)
        .await;
}

/// Vote equivocation test with 7 nodes.
///
/// Nodes 1 and 2 share validator key. Node 2 uses `PrevoteRandom`, node 1 votes normally.
///
/// Need five honest nodes (3-7) so that
///  * equivocator holds < 1/3 of total VP
///  * no single honest node has >2/3 of total VP, so needs to collect votes from others
#[tokio::test]
pub async fn equivocation_two_vals_same_key_vote() {
    // Nodes 1 and 2 share a validator key to induce vote equivocation.
    let params = TestParams {
        shared_key_group: HashSet::from([1, 2]),
        target_time: Some(TARGET_TIME),
        ..Default::default()
    };
    let mut test = TestBuilder::<()>::new();

    // Node 1  - Byzantine
    test.add_node().start().success();

    // Node 2  - Byzantine: same validator key as node 1 and prevotes for random values.
    test.add_node()
        .with_middleware(PrevoteRandom)
        .start()
        .success();

    // Nodes 3 to 6 (honest)
    for _ in 3..=6 {
        test.add_node().start().success();
    }

    // Node 7 (honest) checks vote equivocation evidence.
    test.add_node()
        .start()
        .on_finalized(move |_c, evidence, _s| {
            check_decided_impl(&evidence);
            let has_vote_equivocation = !evidence.votes.is_empty();
            let result = if has_vote_equivocation {
                HandlerResult::ContinueTest
            } else {
                HandlerResult::WaitForNextEvent
            };
            Ok(result)
        })
        .success();

    test.build()
        .run_with_params(Duration::from_secs(30), params)
        .await;
}

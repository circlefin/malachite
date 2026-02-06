use std::{collections::HashSet, time::Duration};

use malachitebft_core_consensus::MisbehaviorEvidence;
use malachitebft_core_types::{Context, Proposal, Vote};
use malachitebft_test_framework::{HandlerResult, TestParams};

use crate::TestBuilder;

fn check_evidence<Ctx: Context>(evidence: &MisbehaviorEvidence<Ctx>) {
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

// Verifies that sharing a validator key across two nodes
// induces equivocation and that decide-time `MisbehaviorEvidence`
// contains double proposals for the equivocator.
// Node 3 checks for proposal equivocation evidence.
#[tokio::test]
pub async fn equivocation_two_vals_same_pk_proposal() {
    // Nodes 1 and 2 share a validator key to induce proposal equivocation
    let params = TestParams {
        shared_key_group: HashSet::from([1, 2]),
        ..Default::default()
    };
    let mut test = TestBuilder::<()>::new();

    // Node 1
    test.add_node().start().success();

    // Node 2 (same validator key as node 1)
    test.add_node().start().success();

    // Node 3 -- checking proposal equivocation evidence
    test.add_node()
        .start()
        .on_decided(|_c, evidence, _s| {
            dbg!(&evidence);

            if evidence.proposals.is_empty() {
                eyre::bail!("Expected proposal equivocation evidence, but none was found");
            }

            check_evidence(&evidence);
            Ok(HandlerResult::ContinueTest)
        })
        .success();

    test.build()
        .run_with_params(Duration::from_secs(30), params)
        .await;
}

// Verifies that sharing a validator key across two nodes
// induces equivocation and that decide-time `MisbehaviorEvidence`
// contains double prevotes for the equivocator.
// Node 3 checks for vote equivocation evidence.
#[tokio::test]
pub async fn equivocation_two_vals_same_pk_vote() {
    // Nodes 1 and 2 share a validator key to induce vote equivocation
    let params = TestParams {
        shared_key_group: HashSet::from([1, 2]),
        ..Default::default()
    };
    let mut test = TestBuilder::<()>::new();

    // Node 1
    test.add_node().start().success();

    // Node 2 (same validator key as node 1)
    test.add_node().start().success();

    // Node 3 -- checking vote equivocation evidence
    test.add_node()
        .start()
        .on_decided(|_c, evidence, _s| {
            dbg!(&evidence);

            if evidence.votes.is_empty() {
                eyre::bail!("Expected vote equivocation evidence, but none was found");
            }

            check_evidence(&evidence);
            Ok(HandlerResult::ContinueTest)
        })
        .success();

    test.build()
        .run_with_params(Duration::from_secs(30), params)
        .await;
}

use std::{collections::HashSet, time::Duration};
use tracing::debug;

use malachitebft_test_framework::TestParams;

use crate::TestBuilder;

#[tokio::test]
pub async fn equivocation_two_vals_same_pk() {
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
        .on_proposal_equivocation_evidence(|_height, _address, (p1, p2), _state| {
            debug!(
                val1 = %p1.message.value.value,
                val2 = %p2.message.value.value,
                "Observed proposal equivocation (values differ)"
            );
            assert_ne!(p1.message.value.value, p2.message.value.value);
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        .on_vote_equivocation_evidence(|_height, _address, (v1, v2), _state| {
            debug!(
                round = ?v1.message.round,
                typ = ?v1.message.typ,
                val1 = ?v1.message.value,
                val2 = ?v2.message.value,
                "Observed vote equivocation"
            );
            assert_eq!(v1.message.round, v2.message.round);
            assert_eq!(v1.message.typ, v2.message.typ);
            assert_ne!(v1.message.value, v2.message.value);
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        .on_decided(|_certificate, evidence, _state| {
            debug!(
                proposals_empty = %evidence.proposals.is_empty(),
                votes_empty = %evidence.votes.is_empty(),
                "Decided: evidence status"
            );
            if !evidence.proposals.is_empty() {
                Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
            } else {
                Ok(malachitebft_test_framework::HandlerResult::WaitForNextEvent)
            }
        })
        .success();

    // Node 4 -- checking vote equivocation evidence
    test.add_node()
        .start()
        .on_proposal_equivocation_evidence(|_height, _address, (p1, p2), _state| {
            debug!(
                val1 = %p1.message.value.value,
                val2 = %p2.message.value.value,
                "Observed proposal equivocation (values differ)"
            );
            assert_ne!(p1.message.value.value, p2.message.value.value);
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        .on_vote_equivocation_evidence(|_height, _address, (v1, v2), _state| {
            debug!(
                round = ?v1.message.round,
                typ = ?v1.message.typ,
                val1 = ?v1.message.value,
                val2 = ?v2.message.value,
                "Observed vote equivocation"
            );
            assert_eq!(v1.message.round, v2.message.round);
            assert_eq!(v1.message.typ, v2.message.typ);
            assert_ne!(v1.message.value, v2.message.value);
            Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
        })
        .on_decided(|_certificate, evidence, _state| {
            debug!(
                proposals_empty = %evidence.proposals.is_empty(),
                votes_empty = %evidence.votes.is_empty(),
                "Decided: evidence status"
            );
            if !evidence.votes.is_empty() {
                Ok(malachitebft_test_framework::HandlerResult::ContinueTest)
            } else {
                Ok(malachitebft_test_framework::HandlerResult::WaitForNextEvent)
            }
        })
        .success();

    test.build()
        .run_with_params(Duration::from_secs(10), params)
        .await;
}

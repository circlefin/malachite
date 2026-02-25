use std::time::Duration;

use malachitebft_core_consensus::MisbehaviorEvidence;
use malachitebft_core_types::{Context, Proposal, Vote};
use malachitebft_engine_byzantine::{ByzantineConfig, Trigger};
use malachitebft_test_framework::HandlerResult;

use crate::TestBuilder;

/// Delay applied to each vote to slow consensus and allow conflicting
/// messages to propagate through the gossip network before a height is decided.
const VOTE_DELAY: Duration = Duration::from_millis(300);

fn validate_evidence<Ctx: Context>(evidence: &MisbehaviorEvidence<Ctx>) {
    for addr in evidence.proposals.iter() {
        let list = evidence.proposals.get(addr).unwrap();
        if let Some((p1, p2)) = list.first() {
            assert_ne!(
                p1.value(),
                p2.value(),
                "Proposal equivocation should have different values"
            );
        }
    }

    for addr in evidence.votes.iter() {
        let list = evidence.votes.get(addr).unwrap();
        if let Some((v1, v2)) = list.first() {
            assert_eq!(
                v1.round(),
                v2.round(),
                "Vote equivocation should be for same round"
            );
            assert_eq!(
                v1.vote_type(),
                v2.vote_type(),
                "Vote equivocation should be for same vote type"
            );
            assert_ne!(
                v1.value(),
                v2.value(),
                "Vote equivocation should have different values"
            );
        }
    }
}

/// A Byzantine node that always equivocates votes should be detected
/// by honest nodes via MisbehaviorEvidence.
#[tokio::test]
pub async fn vote_equivocation_detected() {
    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine — equivocates votes on every message
    test.add_node()
        .with_voting_power(1)
        .start()
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig {
                equivocate_votes: Some(Trigger::Always),
                seed: Some(42),
                ..Default::default()
            });
        })
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .success();

    // Nodes 2-3: Honest validators
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .wait_until(3)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .wait_until(3)
        .success();

    // Node 4: Honest validator that checks for vote equivocation evidence
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .on_finalized(|_cert, evidence, _state| {
            if evidence.votes.is_empty() {
                Ok(HandlerResult::WaitForNextEvent)
            } else {
                validate_evidence(&evidence);
                Ok(HandlerResult::ContinueTest)
            }
        })
        .success();

    test.build().run(Duration::from_secs(60)).await;
}

/// A Byzantine node that always equivocates proposals should be detected
/// by honest nodes via MisbehaviorEvidence.
///
/// NOTE: Currently ignored because the conflicting proposal sent via the
/// gossip network arrives at honest nodes after they have already decided
/// the height, so it gets filtered out before reaching the ProposalKeeper.
/// This is a known limitation of gossip-based equivocation detection.
#[tokio::test]
#[ignore]
pub async fn proposal_equivocation_detected() {
    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine — equivocates proposals on every message
    test.add_node()
        .with_voting_power(10)
        .start()
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig {
                equivocate_proposals: Some(Trigger::Always),
                seed: Some(42),
                ..Default::default()
            });
        })
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .success();

    // Nodes 2-3: Honest validators
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .wait_until(5)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .wait_until(5)
        .success();

    // Node 4: Honest validator that checks for proposal equivocation evidence
    test.add_node()
        .with_voting_power(10)
        .start()
        .on_vote(|_v, _s| Ok(HandlerResult::SleepAndContinueTest(VOTE_DELAY)))
        .on_finalized(|_cert, evidence, _state| {
            if evidence.proposals.is_empty() {
                Ok(HandlerResult::WaitForNextEvent)
            } else {
                validate_evidence(&evidence);
                Ok(HandlerResult::ContinueTest)
            }
        })
        .success();

    test.build().run(Duration::from_secs(90)).await;
}

/// A Byzantine node that drops all its proposals should not prevent
/// the honest majority from making progress.
#[tokio::test]
pub async fn proposal_dropping_liveness() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine — drops all outgoing proposals
    test.add_node()
        .with_voting_power(1)
        .start()
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig {
                drop_proposals: Some(Trigger::Always),
                seed: Some(42),
                ..Default::default()
            });
        })
        .success();

    // Nodes 2-4: Honest validators that should still make progress
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(60)).await;
}

/// A Byzantine node that drops all its votes should not prevent
/// the honest majority from making progress (Byzantine node has
/// minority voting power).
#[tokio::test]
pub async fn vote_dropping_liveness() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine — drops all outgoing votes
    test.add_node()
        .with_voting_power(1)
        .start()
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig {
                drop_votes: Some(Trigger::Always),
                seed: Some(42),
                ..Default::default()
            });
        })
        .success();

    // Nodes 2-4: Honest validators that should still make progress
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(60)).await;
}

/// A Byzantine node performing an amnesia attack (ignoring voting locks)
/// should not prevent the honest majority from making progress.
#[tokio::test]
pub async fn amnesia_attack_liveness() {
    const TARGET_HEIGHT: u64 = 3;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine — ignores voting locks (amnesia attack)
    test.add_node()
        .with_voting_power(1)
        .start()
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig {
                ignore_locks: true,
                ..Default::default()
            });
        })
        .success();

    // Nodes 2-4: Honest validators that should still make progress
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(60)).await;
}

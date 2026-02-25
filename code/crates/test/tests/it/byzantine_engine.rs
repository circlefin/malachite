use std::time::Duration;

use arc_malachitebft_test::middleware::Middleware;
use malachitebft_core_consensus::MisbehaviorEvidence;
use malachitebft_core_types::{Context, LinearTimeouts, Proposal, Vote};
use malachitebft_engine::util::events::Event;
use malachitebft_engine_byzantine::{ByzantineConfig, Trigger};
use malachitebft_test_framework::{Expected, HandlerResult};

use crate::{Height, TestBuilder, TestContext};

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

/// Short timeouts so that rounds cycle quickly in the stall test.
#[derive(Copy, Clone, Debug)]
struct ShortTimeouts;

impl Middleware for ShortTimeouts {
    fn get_timeouts(
        &self,
        _ctx: &TestContext,
        _current_height: Height,
        _height: Height,
    ) -> Option<LinearTimeouts> {
        Some(LinearTimeouts {
            propose: Duration::from_millis(500),
            propose_delta: Duration::from_millis(100),
            prevote: Duration::from_millis(200),
            prevote_delta: Duration::from_millis(100),
            precommit: Duration::from_millis(200),
            precommit_delta: Duration::from_millis(100),
            rebroadcast: Duration::from_millis(500),
        })
    }
}

/// When 2 out of 4 nodes with equal voting power are Byzantine and drop all
/// their votes, honest nodes hold only 50% of the voting power — below the
/// 2/3 quorum threshold required by Tendermint consensus. This means honest
/// nodes can never form a commit certificate and consensus must stall.
///
/// The test verifies that honest nodes observe multiple rebroadcast cycles
/// at height 1 without ever deciding, confirming that the BFT fault threshold
/// (f < n/3) is respected: exceeding it prevents liveness.
#[tokio::test]
pub async fn two_byzantine_of_four_stalls_consensus() {
    /// Number of rebroadcast events an honest node must observe at
    /// height 1 before we conclude consensus has stalled. Multiple
    /// rebroadcast cycles with no decision is evidence of a liveness failure.
    const REBROADCAST_THRESHOLD: usize = 3;

    let mut test = TestBuilder::<usize>::new();

    // Nodes 1-2: Byzantine — drop all outgoing votes.
    // Together they hold 50% of the voting power, so the remaining honest
    // nodes cannot reach the >2/3 quorum by themselves.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine = Some(ByzantineConfig {
                    drop_votes: Some(Trigger::Always),
                    seed: Some(42),
                    ..Default::default()
                });
            })
            .success();
    }

    // Nodes 3-4: Honest validators.
    // Each waits for several vote rebroadcasts (proof that the node is stuck
    // at the prevote step without enough voting power for quorum), then asserts
    // that zero decisions have been made.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .with_middleware(ShortTimeouts)
            .on_event(move |event, rebroadcasts: &mut usize| {
                if let Event::RepublishVote(_) = event {
                    *rebroadcasts += 1;
                    if *rebroadcasts >= REBROADCAST_THRESHOLD {
                        return Ok(HandlerResult::ContinueTest);
                    }
                }
                Ok(HandlerResult::WaitForNextEvent)
            })
            .expect_decisions(Expected::Exactly(0))
            .success();
    }

    test.build().run(Duration::from_secs(60)).await;
}

/// When 2 out of 4 nodes are completely silent (dropping both votes and
/// proposals), honest nodes hold only 50% of the voting power and cannot
/// reach the >2/3 quorum. This is functionally equivalent to the vote-dropping
/// test but exercises the combined silence behavior.
#[tokio::test]
pub async fn two_silent_of_four_stalls_consensus() {
    const REBROADCAST_THRESHOLD: usize = 3;

    let mut test = TestBuilder::<usize>::new();

    // Nodes 1-2: Completely silent Byzantine nodes.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine = Some(ByzantineConfig {
                    drop_votes: Some(Trigger::Always),
                    drop_proposals: Some(Trigger::Always),
                    seed: Some(42),
                    ..Default::default()
                });
            })
            .success();
    }

    // Nodes 3-4: Honest validators.
    // Without proposals or votes from the Byzantine half, the honest nodes
    // have no chance of reaching quorum.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .with_middleware(ShortTimeouts)
            .on_event(move |event, rebroadcasts: &mut usize| {
                if let Event::RepublishVote(_) = event {
                    *rebroadcasts += 1;
                    if *rebroadcasts >= REBROADCAST_THRESHOLD {
                        return Ok(HandlerResult::ContinueTest);
                    }
                }
                Ok(HandlerResult::WaitForNextEvent)
            })
            .expect_decisions(Expected::Exactly(0))
            .success();
    }

    test.build().run(Duration::from_secs(60)).await;
}

/// When 2 out of 4 nodes drop proposals, honest proposer rounds still
/// succeed because all 4 nodes vote normally. The network loses half its
/// proposer rounds but liveness is preserved.
#[tokio::test]
pub async fn two_proposal_droppers_of_four_still_progresses() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Nodes 1-2: Byzantine — drop all proposals but vote normally.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .add_config_modifier(|config| {
                config.byzantine = Some(ByzantineConfig {
                    drop_proposals: Some(Trigger::Always),
                    seed: Some(42),
                    ..Default::default()
                });
            })
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    // Nodes 3-4: Honest validators.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(60)).await;
}

/// When 2 out of 4 equal-power nodes equivocate their votes, honest nodes
/// detect the misbehavior via `MisbehaviorEvidence`. The equivocators'
/// first vote still counts toward quorum, so consensus can make progress
/// while the evidence is collected.
#[tokio::test]
pub async fn two_vote_equivocators_of_four_detected() {
    let mut test = TestBuilder::<()>::new();

    // Nodes 1-2: Byzantine — equivocate every vote.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
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
    }

    // Nodes 3-4: Honest validators that check for equivocation evidence.
    for _ in 0..2 {
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
    }

    test.build().run(Duration::from_secs(60)).await;
}

/// When 2 out of 4 nodes perform an amnesia attack (ignoring voting locks),
/// consensus still progresses because all 4 nodes vote (just potentially
/// for inconsistent values). With 100% of voting power visible, quorum is
/// reachable. The amnesia attack alone cannot cause a safety violation
/// without also equivocating.
#[tokio::test]
pub async fn two_amnesia_of_four_still_progresses() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Nodes 1-2: Byzantine — ignore voting locks.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .add_config_modifier(|config| {
                config.byzantine = Some(ByzantineConfig {
                    ignore_locks: true,
                    ..Default::default()
                });
            })
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    // Nodes 3-4: Honest validators.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .start()
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(60)).await;
}

use std::time::Duration;

use arc_malachitebft_test::middleware::Middleware;
use malachitebft_core_types::LinearTimeouts;
use malachitebft_engine::util::events::Event;
use malachitebft_engine_byzantine::{ByzantineConfig, Trigger};
use malachitebft_test_framework::{Expected, HandlerResult};

use crate::{equivocation, Height, TestBuilder, TestContext};

/// Short timeouts so that rounds cycle quickly, mainly for the stall tests.
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
            propose: Duration::from_millis(200),
            propose_delta: Duration::from_millis(50),
            prevote: Duration::from_millis(100),
            prevote_delta: Duration::from_millis(50),
            precommit: Duration::from_millis(100),
            precommit_delta: Duration::from_millis(50),
            rebroadcast: Duration::from_millis(200),
        })
    }
}

/// When all 4 equal-power nodes equivocate their votes, they should detect the
/// misbehavior at every height. The equivocators' first vote still counts
/// towards a quorum, so consensus should make progress while the evidence is
/// collected.
#[tokio::test]
pub async fn all_vote_equivocators_detected_and_still_progress() {
    let mut test = TestBuilder::<()>::new();

    for _ in 0..4 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine =
                    Some(ByzantineConfig::new(Some(42)).with_equivocate_votes(Trigger::Always));
            })
            .on_finalized(|_cert, evidence, _state| {
                assert!(
                    !evidence.votes.is_empty(),
                    "Vote evidence should not be empty"
                );
                equivocation::check_decided_impl(&evidence);
                Ok(HandlerResult::ContinueTest)
            })
            .start()
            .wait_until(5)
            .success();
    }

    test.build().run(Duration::from_secs(30)).await;
}

/// A single validator not dropping proposals is enough for consensus to make
/// progress.
#[tokio::test]
pub async fn single_proposer_not_equivocating_makes_progress() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Nodes 1-3: Byzantine, equivocate their proposal on every message
    for _ in 0..3 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine =
                    Some(ByzantineConfig::new(Some(42)).with_equivocate_proposals(Trigger::Always));
            })
            .start()
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    // Node 4: Honest validator that makes progress
    test.add_node()
        .with_voting_power(10)
        .with_middleware(ShortTimeouts)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await;
}

/// A single Byzantine node that drops all its votes should not prevent the
/// honest majority from making progress.
#[tokio::test]
pub async fn single_vote_dropper_makes_progress() {
    const TARGET_HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Byzantine, drops all outgoing votes
    test.add_node()
        .with_voting_power(10)
        .with_middleware(ShortTimeouts)
        .add_config_modifier(|config| {
            config.byzantine =
                Some(ByzantineConfig::new(Some(42)).with_drop_votes(Trigger::Always));
        })
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    // Nodes 2-4: Honest validators that should still make progress
    for _ in 0..3 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .start()
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    test.build().run(Duration::from_secs(30)).await;
}

/// When more than 1/3 of the validators drop  all votes, consensus stalls.
/// No even the liveness protocol can ensure liveness, since votes are not
/// received at all. In this test, 2 out of 4 validators drop all votes.
/// their votes, the honest nodes holding 50% of the voting power should never
/// decide and consensus must stall.
///
/// The test verifies that honest nodes observe multiple rebroadcast cycles at
/// height 1 without ever deciding, confirming that the BFT fault threshold
/// (f < n/3) is necessary: exceeding it breaks liveness.
#[tokio::test]
pub async fn two_vote_droppers_of_four_stalls_consensus() {
    /// Number of rebroadcast events an honest node must observe at
    /// height 1 before we conclude consensus has stalled. Multiple
    /// rebroadcast cycles with no decision is evidence of a liveness failure.
    const REBROADCAST_THRESHOLD: usize = 3;

    let mut test = TestBuilder::<usize>::new();

    // Nodes 1-2: Byzantine, drop all outgoing votes.
    // Together they hold 50% of the voting power, so the remaining honest
    // nodes cannot reach the >2/3 quorum by themselves.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine =
                    Some(ByzantineConfig::new(Some(42)).with_drop_votes(Trigger::Always));
            })
            .start()
            .success();
    }

    // Nodes 3-4: Honest validators.
    // Each waits for several vote rebroadcasts (proof that the node is stuck
    // at the prevote step without enough voting power for quorum), then asserts
    // that zero decisions have been made.
    for _ in 0..2 {
        test.add_node()
            .with_voting_power(10)
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
            .start()
            .expect_decisions(Expected::Exactly(0))
            .success();
    }

    test.build().run(Duration::from_secs(30)).await;
}

/// When 3 out of 4 nodes drop all their proposals, the lone honest validator
/// that still broadcasts proposals is enough to keep consensus moving. The
/// network loses three proposer rounds out of four, but all validators still
/// vote normally once that single honest proposer gets a turn.
#[tokio::test]
pub async fn three_proposal_droppers_of_four_still_progresses() {
    const TARGET_HEIGHT: u64 = 3;

    let mut test = TestBuilder::<()>::new();

    // Nodes 1-3: Byzantine, drop all proposals but vote normally.
    for _ in 0..3 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine =
                    Some(ByzantineConfig::new(Some(42)).with_drop_proposals(Trigger::Always));
            })
            .start()
            .wait_until(TARGET_HEIGHT)
            .success();
    }

    // Node 4: Honest validator. Its proposer turns are sufficient for liveness.
    test.add_node()
        .with_voting_power(10)
        .with_middleware(ShortTimeouts)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(30)).await;
}

/// Scenario (4 equal-weight, power 10 each; f = 13, 2f+1 = 27):
/// - Node 1, Node 3: honest. Node 1 is the round-0 proposer.
/// - Node 2: `force_precommit_nil` at height 1, rounds 0, 1, 2. It still
///   prevotes the value so the polka (and hence a valid `pol_round` for the
///   round-1 and round-2 reproposers) still forms in each round, but its
///   precommits never count toward a non-nil quorum.
/// - Node 4: `drop_inbound_proposals` at height 1, rounds 0 and 1. Node 4
///   still receives the proposal parts (so it emits
///   `Value(height=1, round=0, pol_round=Nil, value)` via restream) but
///   the consensus engine never sees the `SignedProposal` messages for rounds 0 and 1,
///   so no `ProposalOnly`/`Full` entry is stored at `(1, 0)` or `(1, 1)`.
///   In round 2 the proposer issues `Proposal(round=2, pol_round=1, value)`.
///   The `FullProposalKeeper` should match that proposal against the
///   `ValueOnly(value)` at `(1, 0)`.
///   Notes:
///    - currently the keeper can only match
///     ProposalOnly(h, r, vr, value_id) <-> ValueOnly(h, r', vr', value_id)
///     if r == r' or vr == r'
///    - Because of this node 4 never applies `DriverInput::Proposal`, never
///   prevotes or precommits `value` in round 2, and cannot decide height 1.
///    - `value_sync` is disabled on node 4 so it must decide via consensus.
#[tokio::test]
pub async fn mux_cross_round_proposal_matches_stored_value_via_restream() {
    const TARGET_HEIGHT: u64 = 2;

    let mut test = TestBuilder::<()>::new();

    // Node 1: honest. Round-0 proposer.
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    // Node 2: forces nil precommits at height 1, rounds 0, 1, 2.
    test.add_node()
        .with_voting_power(10)
        .add_config_modifier(|config| {
            config.byzantine = Some(ByzantineConfig::new(Some(42)).with_force_precommit_nil(
                Trigger::AtHeightsAndRounds {
                    heights: vec![1],
                    rounds: vec![0, 1, 2],
                },
            ));
        })
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    // Node 3: honest.
    test.add_node()
        .with_voting_power(10)
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    // Node 4: drops inbound `SignedProposal` messages at height 1, rounds 0
    // and 1. Parts still flow through, so it builds `ValueOnly(v)` at
    // `(1, 0)` via verbatim restream, but the proposal messages for those
    // rounds never reach consensus. From round 2 onwards it is honest.
    test.add_node()
        .with_voting_power(10)
        .add_config_modifier(|config| {
            // Prevent node 4 from catching up post-decision via value sync.
            config.value_sync.enabled = false;
            config.byzantine = Some(ByzantineConfig::new(Some(42)).with_drop_inbound_proposals(
                Trigger::AtHeightsAndRounds {
                    heights: vec![1],
                    rounds: vec![0, 1],
                },
            ));
        })
        .start()
        .wait_until(TARGET_HEIGHT)
        .success();

    test.build().run(Duration::from_secs(10)).await;
}

/// Amnesia (ignoring voting locks) should not prevent progress, even when
/// affecting all nodes.
///
/// Note that if nodes manage to receive proposals and votes before the timeout,
/// consensus is reached in round 0. Amnesia starts showing effects from round 1.
///
/// This test also checks agreement: all nodes must decide the same value at
/// each height. With all nodes being Byzantine, the hidden-lock attack
/// described in <https://github.com/circlefin/malachite/issues/956> could
/// theoretically produce disagreement.
#[tokio::test]
pub async fn amnesia_makes_progress() {
    let mut test = TestBuilder::<()>::new();

    for _ in 0..4 {
        test.add_node()
            .with_voting_power(10)
            .with_middleware(ShortTimeouts)
            .add_config_modifier(|config| {
                config.byzantine =
                    Some(ByzantineConfig::new(Some(42)).with_ignore_locks(Trigger::Always));
            })
            .start()
            .wait_until(5)
            .success();
    }

    test.build().run(Duration::from_secs(30)).await;
}

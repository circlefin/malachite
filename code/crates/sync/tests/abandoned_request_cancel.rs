//! Abandoned outbound sync requests must cancel their engine-side timers so
//! peers are not asymmetrically scored, and must release the client-latency
//! start instant they own in `Metrics`.

use std::collections::BTreeSet;

use arc_malachitebft_sync::handle::Input;
use arc_malachitebft_sync::{
    Effect, Error, HeightStartType, Metrics, OutboundRequestId, PeerId, PendingRequestEntry, State,
    Status, ValueResponse,
};
use arc_malachitebft_test::{Height, TestContext};

mod common;
use common::{drive_input, drive_input_numbering_requests, make_raw_value, make_test_state};

/// Resume `SendValueRequest` with synthetic ids so partial-response suffix
/// re-requests (and other request passes) can complete.
fn drive_input_with_retries(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    input: Input<TestContext>,
) -> Result<Vec<Effect<TestContext>>, Error<TestContext>> {
    let mut req_counter = 0u64;
    drive_input_numbering_requests(state, metrics, input, "retry_req", &mut req_counter)
}

/// `instant_request_sent` is private, so the entry is observed through the
/// public `value_response_received`. That is the production release path, not a
/// getter: it consumes the entry. Hence `take_` — call at most one per request
/// id per test.
fn take_latency_entry_expect_absent(metrics: &Metrics, request_id: &str) {
    assert!(
        metrics
            .value_response_received(&OutboundRequestId::new(request_id))
            .is_none(),
        "latency entry for {request_id} must be released when the request is retired"
    );
}

fn take_latency_entry_expect_present(metrics: &Metrics, request_id: &str) {
    assert!(
        metrics
            .value_response_received(&OutboundRequestId::new(request_id))
            .is_some(),
        "latency entry for {request_id} must still be held"
    );
}

fn asserts_cancel_for(effects: &[Effect<TestContext>], request_id: &str) {
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::CancelValueRequest(id, _) if id == &OutboundRequestId::new(request_id)
        )),
        "expected CancelValueRequest for {request_id}: {effects:?}"
    );
}

#[test]
fn restart_clears_inflight_request_emits_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(15);

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(14),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    let effects = drive_input(
        &mut state,
        &metrics,
        Input::StartedHeight(Height::new(10), HeightStartType::Restart),
    )
    .unwrap();

    assert!(state.pending_requests.is_empty());
    asserts_cancel_for(&effects, "req1");
}

#[test]
fn restart_clears_reservation_emits_no_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(15);

    // A reservation already had its engine timer cancelled when the response
    // arrived; clearing it must not emit CancelValueRequest.
    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(14),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: false,
        },
    );

    let effects = drive_input(
        &mut state,
        &metrics,
        Input::StartedHeight(Height::new(10), HeightStartType::Restart),
    )
    .unwrap();

    assert!(state.pending_requests.is_empty());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::CancelValueRequest(..))),
        "Restart must not cancel reservations that already released their timer: {effects:?}"
    );
}

#[test]
fn decided_prunes_inflight_request_emits_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(16);

    // Range is fully at or below the tip after Decided(15), so prune drops it.
    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(15),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    let effects = drive_input(&mut state, &metrics, Input::Decided(Height::new(15))).unwrap();

    assert!(
        state.pending_requests.is_empty(),
        "Fully validated in-flight request must be pruned"
    );
    asserts_cancel_for(&effects, "req1");
}

#[test]
fn decided_prunes_reservation_emits_no_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(16);

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(15),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: false,
        },
    );

    let effects = drive_input(&mut state, &metrics, Input::Decided(Height::new(15))).unwrap();

    assert!(state.pending_requests.is_empty());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::CancelValueRequest(..))),
        "Prune must not cancel reservations that already released their timer: {effects:?}"
    );
}

#[test]
fn started_height_non_restart_prunes_inflight_request_emits_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    // Before Start(16), tip is still below the pending range.
    state.tip_height = Height::new(9);
    state.sync_height = Height::new(16);

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(15),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    // Non-restart sets tip = height - 1 = 15, then prunes (not clears).
    let effects = drive_input(
        &mut state,
        &metrics,
        Input::StartedHeight(Height::new(16), HeightStartType::Start),
    )
    .unwrap();

    assert_eq!(state.tip_height, Height::new(15));
    assert!(
        !state
            .pending_requests
            .contains_key(&OutboundRequestId::new("req1")),
        "Fully validated in-flight request must be pruned on non-restart start"
    );
    asserts_cancel_for(&effects, "req1");
}

#[test]
fn partial_response_prunes_stale_inflight_request_emits_cancel() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    // Tip already past the stale request's range; the answering request is still ahead.
    state.tip_height = Height::new(12);
    state.sync_height = Height::new(18);
    state.consensus_height = Height::new(13);

    let peer = PeerId::random();
    state.peers.insert(
        peer,
        Status {
            peer_id: peer,
            tip_height: Height::new(30),
            history_min_height: Height::new(1),
        },
    );

    state.pending_requests.insert(
        OutboundRequestId::new("stale"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(12),
            peer,
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );
    state.pending_requests.insert(
        OutboundRequestId::new("answering"),
        PendingRequestEntry {
            range: Height::new(13)..=Height::new(17),
            peer,
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    // Prefix of the answering range: updates it to a reservation, then prune
    // drops the stale still-inflight entry whose range is fully at/below tip.
    let response = ValueResponse::new(
        Height::new(13),
        vec![make_raw_value(13), make_raw_value(14)],
    );

    let effects = drive_input_with_retries(
        &mut state,
        &metrics,
        Input::ValueResponse(OutboundRequestId::new("answering"), peer, Some(response)),
    )
    .unwrap();

    assert!(
        !state
            .pending_requests
            .contains_key(&OutboundRequestId::new("stale")),
        "Stale in-flight request fully at/below tip must be pruned on partial response"
    );
    asserts_cancel_for(&effects, "stale");
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::CancelValueRequest(id, _) if id == &OutboundRequestId::new("answering")
        )),
        "The answering request becomes a reservation and must not be cancelled: {effects:?}"
    );
}

/// Guard for the observation technique used by the tests below: a seeded entry
/// reads back as `Some` exactly once, then as `None`.
#[test]
fn seeded_latency_entry_is_observable_exactly_once() {
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    take_latency_entry_expect_present(&metrics, "req1");
    take_latency_entry_expect_absent(&metrics, "req1");
}

/// The success path must observe the latency before releasing the entry, or the
/// `value_client_latency` histogram silently stops being fed. Asserts on the
/// peer score, since `on_valid_value_response` scores only inside the
/// `Some(response_time)` branch.
#[test]
fn full_response_observes_latency_before_releasing_entry() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(13);
    state.consensus_height = Height::new(10);

    let peer = PeerId::random();
    state.peers.insert(
        peer,
        Status {
            peer_id: peer,
            tip_height: Height::new(30),
            history_min_height: Height::new(1),
        },
    );

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(12),
            peer,
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    // Full response: covers the whole requested range.
    let response = ValueResponse::new(
        Height::new(10),
        vec![make_raw_value(10), make_raw_value(11), make_raw_value(12)],
    );

    drive_input_with_retries(
        &mut state,
        &metrics,
        Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
    )
    .unwrap();

    assert!(
        state.peer_scorer.get_scores().contains_key(&peer),
        "the latency must be observed before the entry is released: \
         a missing peer score means value_response_received saw no entry"
    );

    // The observation itself consumed the entry.
    take_latency_entry_expect_absent(&metrics, "req1");
}

/// `on_invalid_value_response` routes to the retry path, whose give-up branch
/// (no eligible peer left) removes the pending request and sends no
/// replacement. The latency entry must be released on that branch too.
#[test]
fn invalid_response_without_retry_peer_releases_latency_entry() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(15);

    // No peer is registered, so `random_peer_with_except` finds nobody and the
    // retry gives up without issuing a replacement request.
    let peer = PeerId::random();
    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(14),
            peer,
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    let effects = drive_input(
        &mut state,
        &metrics,
        Input::ValueResponse(OutboundRequestId::new("req1"), peer, None),
    )
    .unwrap();

    assert!(
        state.pending_requests.is_empty(),
        "the invalid response must drop the pending request"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::SendValueRequest(..))),
        "the give-up branch must not issue a replacement request: {effects:?}"
    );

    take_latency_entry_expect_absent(&metrics, "req1");
}

/// The restart path clears every pending request through
/// `clear_pending_requests`, whose ids flow into `cancel_value_requests`.
#[test]
fn restart_clear_releases_latency_entry() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(15);

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(14),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    let effects = drive_input(
        &mut state,
        &metrics,
        Input::StartedHeight(Height::new(10), HeightStartType::Restart),
    )
    .unwrap();

    assert!(state.pending_requests.is_empty());
    asserts_cancel_for(&effects, "req1");

    take_latency_entry_expect_absent(&metrics, "req1");
}

/// The `on_decided` prune path: the request's whole range is already validated,
/// so the height it was keyed by is never requested again.
#[test]
fn decided_prune_releases_latency_entry() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(16);

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(15),
            peer: PeerId::random(),
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    let effects = drive_input(&mut state, &metrics, Input::Decided(Height::new(15))).unwrap();

    assert!(state.pending_requests.is_empty());
    asserts_cancel_for(&effects, "req1");

    take_latency_entry_expect_absent(&metrics, "req1");
}

/// A retry releases the abandoned request's entry and leaves the replacement's
/// own intact: the release is scoped to the abandoned id.
#[test]
fn retry_releases_old_entry_and_keeps_replacement_entry() {
    let mut state = make_test_state();
    state.started = true;
    let metrics = Metrics::new(std::time::Duration::from_secs(10));

    state.tip_height = Height::new(9);
    state.sync_height = Height::new(15);

    // Two peers, so excluding the one that served the invalid response still
    // leaves an eligible peer for the replacement request.
    let bad_peer = PeerId::random();
    let good_peer = PeerId::random();
    for peer in [bad_peer, good_peer] {
        state.peers.insert(
            peer,
            Status {
                peer_id: peer,
                tip_height: Height::new(30),
                history_min_height: Height::new(1),
            },
        );
    }

    state.pending_requests.insert(
        OutboundRequestId::new("req1"),
        PendingRequestEntry {
            range: Height::new(10)..=Height::new(14),
            peer: bad_peer,
            excluded_peers: BTreeSet::new(),
            inflight: true,
        },
    );

    metrics.value_request_sent(&OutboundRequestId::new("req1"));

    let effects = drive_input_with_retries(
        &mut state,
        &metrics,
        Input::ValueResponse(OutboundRequestId::new("req1"), bad_peer, None),
    )
    .unwrap();

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SendValueRequest(..))),
        "the retry must issue a replacement request: {effects:?}"
    );
    assert!(
        state
            .pending_requests
            .contains_key(&OutboundRequestId::new("retry_req1")),
        "the replacement request must be tracked: {:?}",
        state.pending_requests
    );

    take_latency_entry_expect_absent(&metrics, "req1");
    take_latency_entry_expect_present(&metrics, "retry_req1");
}

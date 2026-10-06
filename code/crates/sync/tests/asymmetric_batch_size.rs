//! A node whose `batch_size` is larger than its peers' must still make
//! progress: the server serves what its own `batch_size` allows and the client
//! takes the short response and asks for the rest.

use std::ops::RangeInclusive;

use arc_malachitebft_sync::handle::Input;
use arc_malachitebft_sync::scoring::SyncResult;
use arc_malachitebft_sync::{
    Config, Effect, HeightStartType, InboundRequestId, Metrics, PeerId, Status, ValueRequest,
    ValueResponse,
};
use arc_malachitebft_test::{Height, TestContext};
use malachitebft_core_types::utils::height::HeightRangeExt;

mod common;
use common::{drive_input, drive_input_numbering_requests, make_raw_value, make_test_state_with};

const CLIENT_BATCH_SIZE: usize = 10;
const SERVER_BATCH_SIZE: usize = 5;

/// Failures to charge the peer before the exchange, to put its score well
/// below neutral.
///
/// A peer starts at the neutral score 0.5, and a partial success of
/// `SERVER_BATCH_SIZE / CLIENT_BATCH_SIZE` lands on exactly 0.5 again. Left at
/// its default score the peer would therefore not move at all, and the final
/// assertion could tell neither a scored response from an unscored one, nor a
/// partial success from a penalty.
const SEED_FAILURES: usize = 20;

fn requested_ranges(effects: &[Effect<TestContext>]) -> Vec<RangeInclusive<Height>> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::SendValueRequest(_, request, _) => Some(request.range.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn server_with_a_smaller_batch_size_serves_a_prefix_and_the_client_advances() {
    // One metrics registry per role, so the two nodes stay distinct subjects.
    let client_metrics = Metrics::new(std::time::Duration::from_secs(10));
    let server_metrics = Metrics::new(std::time::Duration::from_secs(10));

    // One request at a time, so the exchange below is a single round trip.
    let mut client = make_test_state_with(
        Config::default()
            .with_batch_size(CLIENT_BATCH_SIZE)
            .with_parallel_requests(1),
    );
    let mut server = make_test_state_with(Config::default().with_batch_size(SERVER_BATCH_SIZE));

    let server_peer = PeerId::random();
    let client_peer = PeerId::random();

    server.tip_height = Height::new(100);
    server.history_min_height = Height::new(1);

    // Start the peer below neutral, so a partial success has to raise the
    // score for the assertion at the end to hold.
    for _ in 0..SEED_FAILURES {
        client
            .peer_scorer
            .update_score(server_peer, SyncResult::Failure);
    }
    let score_before = client.peer_scorer.get_score(&server_peer);
    assert!(
        score_before < 0.5,
        "the peer must start below neutral for the score assertion to mean anything"
    );

    // The client is at height 1 and learns the server holds everything up to 100.
    let mut next_id = 0;
    drive_input_numbering_requests(
        &mut client,
        &client_metrics,
        Input::StartedHeight(Height::new(1), HeightStartType::Start),
        "req",
        &mut next_id,
    )
    .unwrap();
    let effects = drive_input_numbering_requests(
        &mut client,
        &client_metrics,
        Input::Status(Status {
            peer_id: server_peer,
            tip_height: Height::new(100),
            history_min_height: Height::new(1),
        }),
        "req",
        &mut next_id,
    )
    .unwrap();

    let ranges = requested_ranges(&effects);
    assert_eq!(ranges.len(), 1, "expected a single request: {effects:?}");
    let requested_range = ranges[0].clone();
    assert_eq!(
        requested_range.len(),
        CLIENT_BATCH_SIZE,
        "the client asks for its own batch size"
    );

    // The server shortens the over-long request to its own batch size.
    let inbound_id = InboundRequestId::new("inbound1");
    let effects = drive_input(
        &mut server,
        &server_metrics,
        Input::ValueRequest(
            inbound_id.clone(),
            client_peer,
            ValueRequest::new(requested_range),
        ),
    )
    .unwrap();

    let served_range = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::GetDecidedValues(_, range, _) => Some(range.clone()),
            _ => None,
        })
        .expect("the request must reach the host instead of being refused");
    assert_eq!(
        served_range,
        Height::new(1)..=Height::new(SERVER_BATCH_SIZE as u64)
    );

    // The host returns the shortened range and the server answers with it.
    let values = served_range
        .clone()
        .iter_heights()
        .map(|height| make_raw_value(height.as_u64()))
        .collect();
    let effects = drive_input(
        &mut server,
        &server_metrics,
        Input::GotDecidedValues(inbound_id, served_range, values),
    )
    .unwrap();
    let response = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::SendValueResponse(_, response, _) => Some(response.clone()),
            _ => None,
        })
        .expect("the server must answer");
    assert_eq!(
        response.start_height,
        Height::new(1),
        "the response must start where the client asked, so the client accepts it"
    );

    // The client takes the short response instead of charging the peer a fault.
    assert_eq!(
        client.pending_requests.len(),
        1,
        "the exchange below assumes the client has exactly one request outstanding"
    );
    let request_id = client
        .pending_requests
        .keys()
        .next()
        .cloned()
        .expect("the client has a pending request");

    let effects = drive_input_numbering_requests(
        &mut client,
        &client_metrics,
        Input::ValueResponse(
            request_id,
            server_peer,
            Some(ValueResponse::new(response.start_height, response.values)),
        ),
        "req",
        &mut next_id,
    )
    .unwrap();

    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::ProcessValueResponse(..))),
        "the short response must be forwarded to consensus: {effects:?}"
    );
    assert!(
        client.peer_scorer.get_score(&server_peer) > score_before,
        "a partial success must raise the score, not penalize the peer"
    );

    // The heights the server did not serve are requested again, so the client
    // is not stalled.
    let ranges = requested_ranges(&effects);
    assert_eq!(ranges.len(), 1, "expected a follow-up request: {effects:?}");
    assert_eq!(
        *ranges[0].start(),
        Height::new(SERVER_BATCH_SIZE as u64 + 1),
        "the follow-up starts at the first height the server did not serve"
    );
}

use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use derive_where::derive_where;
use tracing::{debug, error, info, warn};

use malachitebft_core_types::{Context, Height};

use crate::co::Co;
use crate::scoring::SyncResult;
use crate::{
    perform, Effect, Error, HeightStartType, InboundRequestId, Metrics, OutboundRequestId, PeerId,
    RawDecidedValue, Request, Resume, State, Status, ValueRequest, ValueResponse,
};

#[derive_where(Debug)]
pub enum Input<Ctx: Context> {
    /// A tick has occurred
    Tick,

    /// A status update has been received from a peer
    Status(Status<Ctx>),

    /// Consensus just started a new height.
    /// The boolean indicates whether this was a restart or a new start.
    StartedHeight(Ctx::Height, HeightStartType),

    /// Consensus just decided on a new value
    Decided(Ctx::Height),

    /// A ValueSync request has been received from a peer
    ValueRequest(InboundRequestId, PeerId, ValueRequest<Ctx>),

    /// A (possibly empty or invalid) ValueSync response has been received
    ValueResponse(OutboundRequestId, PeerId, Option<ValueResponse<Ctx>>),

    /// Got a response from the application to our `GetDecidedValues` request
    GotDecidedValues(
        InboundRequestId,
        RangeInclusive<Ctx::Height>,
        Vec<RawDecidedValue<Ctx>>,
    ),

    /// A request for a value timed out
    SyncRequestTimedOut(OutboundRequestId, PeerId, Request<Ctx>),

    /// We received an invalid value (either certificate or value)
    InvalidValue(PeerId, Ctx::Height),

    /// An error occurred while processing a value
    ValueProcessingError(PeerId, Ctx::Height),
}

pub async fn handle<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    input: Input<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match input {
        Input::Tick => on_tick(co, state, metrics).await,

        Input::Status(status) => on_status(co, state, metrics, status).await,

        Input::StartedHeight(height, restart) => {
            on_started_height(co, state, metrics, height, restart).await
        }

        Input::Decided(height) => on_decided(state, metrics, height).await,

        Input::ValueRequest(request_id, peer_id, request) => {
            on_value_request(co, state, metrics, request_id, peer_id, request).await
        }

        Input::ValueResponse(request_id, peer_id, Some(response)) => {
            let start = response.start_height;
            let end = response.end_height().unwrap_or(start);
            let range_len = end.as_u64() - start.as_u64() + 1;

            // Check if the response is valid. A valid response starts at the
            // requested start height, has at least one value, and no more than
            // the requested range.
            if let Some((requested_range, stored_peer_id)) = state.pending_requests.get(&request_id)
            {
                if stored_peer_id != &peer_id {
                    warn!(
                        %request_id, peer.actual = %peer_id, peer.expected = %stored_peer_id,
                        "Received response from different peer than expected"
                    );
                    return on_invalid_value_response(co, state, metrics, request_id, peer_id)
                        .await;
                }

                let is_valid = start.as_u64() == requested_range.start().as_u64()
                    && start.as_u64() <= end.as_u64()
                    && end.as_u64() <= requested_range.end().as_u64()
                    && response.values.len() as u64 == range_len;
                if is_valid {
                    return on_value_response(co, state, metrics, request_id, peer_id, response)
                        .await;
                } else {
                    warn!(%request_id, %peer_id, "Received request for wrong range of heights: expected {}..={} ({} values), got {}..={} ({} values)",
                        requested_range.start().as_u64(), requested_range.end().as_u64(), range_len,
                        start.as_u64(), end.as_u64(), response.values.len() as u64);
                    return on_invalid_value_response(co, state, metrics, request_id, peer_id)
                        .await;
                }
            } else {
                warn!(%request_id, %peer_id, "Received response for unknown request ID");
            }

            Ok(())
        }

        Input::ValueResponse(request_id, peer_id, None) => {
            on_invalid_value_response(co, state, metrics, request_id, peer_id).await
        }

        Input::GotDecidedValues(request_id, range, values) => {
            on_got_decided_values(co, state, metrics, request_id, range, values).await
        }

        Input::SyncRequestTimedOut(request_id, peer_id, request) => {
            on_sync_request_timed_out(co, state, metrics, request_id, peer_id, request).await
        }

        Input::InvalidValue(peer, value) => on_invalid_value(co, state, metrics, peer, value).await,

        Input::ValueProcessingError(peer, height) => {
            on_value_processing_error(co, state, metrics, peer, height).await
        }
    }
}

pub async fn on_tick<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(height.tip = %state.tip_height, "Broadcasting status");

    perform!(
        co,
        Effect::BroadcastStatus(state.tip_height, Default::default())
    );

    if let Some(inactive_threshold) = state.config.inactive_threshold {
        // If we are at or above the inactive threshold, we can prune inactive peers.
        state
            .peer_scorer
            .reset_inactive_peers_scores(inactive_threshold);
    }

    debug!("Peer scores: {:#?}", state.peer_scorer.get_scores());

    Ok(())
}

pub async fn on_status<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    status: Status<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let peer_id = status.peer_id;
    let peer_height = status.tip_height;

    debug!(peer.id = %peer_id, peer.height = %peer_height, "Received peer status");

    state.update_status(status);

    if !state.started {
        // Consensus has not started yet, no need to sync (yet).
        return Ok(());
    }

    if peer_height >= state.sync_height {
        warn!(
            height.tip = %state.tip_height,
            height.sync = %state.sync_height,
            height.peer = %peer_height,
            "SYNC REQUIRED: Falling behind"
        );

        // We are lagging behind on one of our peers at least.
        // Request values from any peer already at or above that peer's height.
        request_values(co, state, metrics).await?;
    }

    Ok(())
}

pub async fn on_started_height<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    start_type: HeightStartType,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%height, is_restart=%start_type.is_restart(), "Consensus started new height");

    state.started = true;

    // The tip is the last decided value.
    state.tip_height = height.decrement().unwrap_or_default();

    // Garbage collect fully-validated requests.
    state.remove_fully_validated_requests();

    if start_type.is_restart() {
        // Consensus is retrying the height, so we should sync starting from it.
        state.sync_height = height;
        // Clear pending requests, as we are restarting the height.
        state.pending_requests.clear();
    } else {
        // If consensus is voting on a height that is currently being synced from a peer, do not update the sync height.
        state.sync_height = max(state.sync_height, height);
    }

    // Trigger potential requests if possible.
    request_values(co, state, metrics).await?;

    Ok(())
}

pub async fn on_decided<Ctx>(
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%height, "Consensus decided on new value");

    state.tip_height = height;

    // Garbage collect fully-validated requests.
    state.remove_fully_validated_requests();

    // The next height to sync should always be higher than the tip.
    if state.sync_height == state.tip_height {
        state.sync_height = state.sync_height.increment();
    }

    Ok(())
}

pub async fn on_value_request<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    peer_id: PeerId,
    request: ValueRequest<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(range = %DisplayRange::<Ctx>(&request.range), %peer_id, "Received request for values");

    metrics.value_request_received(request.range.start().as_u64());

    perform!(
        co,
        Effect::GetDecidedValues(request_id, request.range, Default::default())
    );

    Ok(())
}

pub async fn on_value_response<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer_id: PeerId,
    response: ValueResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let start = response.start_height;
    debug!(start = %start, num_values = %response.values.len(), %peer_id, "Received response from peer");

    if let Some(response_time) = metrics.value_response_received(start.as_u64()) {
        state.peer_scorer.update_score_with_metrics(
            peer_id,
            SyncResult::Success(response_time),
            &metrics.scoring,
        );
    }

    // If the response contains a prefix of the requested values, re-request the remaining values.
    if let Some((requested_range, stored_peer_id)) = state.pending_requests.get(&request_id) {
        if stored_peer_id != &peer_id {
            warn!(
                %request_id, peer.actual = %peer_id, peer.expected = %stored_peer_id,
                "Received response from different peer than expected"
            );
        }
        let range_len = requested_range.end().as_u64() - requested_range.start().as_u64() + 1;
        if (response.values.len() as u64) < range_len {
            re_request_values_from_peer_except(co, state, metrics, request_id, None).await?;
        }
    }

    Ok(())
}

pub async fn on_invalid_value_response<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer_id: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%request_id, %peer_id, "Received invalid response");

    state.peer_scorer.update_score(peer_id, SyncResult::Failure);

    // We do not trust the response, so we remove the pending request and re-request
    // the whole range from another peer.
    re_request_values_from_peer_except(co, state, metrics, request_id, Some(peer_id)).await?;

    Ok(())
}

pub async fn on_got_decided_values<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    range: RangeInclusive<Ctx::Height>,
    values: Vec<RawDecidedValue<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(range = %DisplayRange::<Ctx>(&range), "Received {} values from host", values.len());

    let start = range.start();
    let end = range.end();

    // Validate response from host
    let batch_size = end.as_u64() - start.as_u64() + 1;
    if batch_size != values.len() as u64 {
        error!(
            "Received {} values from host, expected {batch_size}",
            values.len()
        )
    }

    // Validate the height of each received value
    let mut height = *start;
    for value in values.clone() {
        if value.certificate.height != height {
            error!(
                "Received from host value for height {}, expected for height {height}",
                value.certificate.height
            );
        }
        height = height.increment();
    }

    debug!(%request_id, range = %DisplayRange::<Ctx>(&range), "Sending response to peer");
    perform!(
        co,
        Effect::SendValueResponse(
            request_id,
            ValueResponse::new(*start, values),
            Default::default()
        )
    );

    metrics.value_response_sent(start.as_u64());

    Ok(())
}

pub async fn on_sync_request_timed_out<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer_id: PeerId,
    request: Request<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match request {
        Request::ValueRequest(value_request) => {
            warn!(%peer_id, range = %DisplayRange::<Ctx>(&value_request.range), "Sync request timed out");

            state.peer_scorer.update_score(peer_id, SyncResult::Timeout);

            metrics.value_request_timed_out(value_request.range.start().as_u64());

            re_request_values_from_peer_except(co, state, metrics, request_id, Some(peer_id))
                .await?;
        }
    };

    Ok(())
}

// When receiving an invalid value, re-request the whole batch from another peer.
async fn on_invalid_value<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%peer_id, %height, "Received invalid value");

    state.peer_scorer.update_score(peer_id, SyncResult::Failure);

    if let Some((request_id, stored_peer_id)) = state.get_request_id_by(height) {
        if stored_peer_id != peer_id {
            warn!(
                %request_id, peer.actual = %peer_id, peer.expected = %stored_peer_id,
                "Received response from different peer than expected"
            );
        }
        re_request_values_from_peer_except(co, state, metrics, request_id, Some(peer_id)).await?;
    } else {
        error!(%peer_id, %height, "Received height of invalid value for unknown request");
    }

    Ok(())
}

async fn on_value_processing_error<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%peer_id, %height, "Error while processing value");

    // NOTE: We do not update the peer score here, as this is an internal error
    //       and not a failure from the peer's side.

    if let Some((request_id, _)) = state.get_request_id_by(height) {
        re_request_values_from_peer_except(co, state, metrics, request_id, None).await?;
    } else {
        error!(%peer_id, %height, "Received height of invalid value for unknown request");
    }

    Ok(())
}

/// Request multiple batches of values in parallel.
async fn request_values<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let max_parallel_requests = max(1, state.config.parallel_requests);

    if state.pending_requests.len() as u64 >= max_parallel_requests {
        info!(
            %max_parallel_requests,
            pending_requests = %state.pending_requests.len(),
            "Maximum number of parallel requests reached, skipping request for values"
        );

        return Ok(());
    };

    while (state.pending_requests.len() as u64) < max_parallel_requests {
        // Find the next uncovered range starting from current sync_height
        let initial_height = state.sync_height;
        let range = find_next_uncovered_range_from::<Ctx>(
            initial_height,
            state.config.batch_size as u64,
            &state.pending_requests,
        );

        // Get a random peer that can provide the values in the range.
        let Some((peer, range)) = state.random_peer_with(&range) else {
            debug!("No peer to request sync from");
            // No connected peer reached this height yet, we can stop syncing here.
            break;
        };

        // Send the request
        let Some((request_id, final_range)) =
            send_request_to_peer(&co, state, metrics, range, peer).await?
        else {
            continue; // Request was skipped (empty range, etc.), try next iteration
        };

        // Store the pending request
        state
            .pending_requests
            .insert(request_id, (final_range.clone(), peer));

        // Update sync_height to the next uncovered height after this range
        let starting_height = final_range.end().increment();
        state.sync_height =
            find_next_uncovered_height::<Ctx>(starting_height, &state.pending_requests);
    }

    Ok(())
}

/// Send a value request to a peer. Returns the request_id and final range if successful.
/// The calling function is responsible for storing the request and updating state.
async fn send_request_to_peer<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    range: RangeInclusive<Ctx::Height>,
    peer: PeerId,
) -> Result<Option<(OutboundRequestId, RangeInclusive<Ctx::Height>)>, Error<Ctx>>
where
    Ctx: Context,
{
    info!(range = %DisplayRange::<Ctx>(&range), peer.id = %peer, "Requesting sync from peer");

    if range.is_empty() {
        warn!(range.sync = %DisplayRange::<Ctx>(&range), %peer, "Range is empty, skipping request");
        return Ok(None);
    }

    // Skip over any heights in the range that are not waiting for a response
    // (meaning that they have been validated by consensus or a peer).
    let range = state.trim_validated_heights(&range);
    if range.is_empty() {
        warn!(%peer, "All values in range {} have been validated, skipping request", DisplayRange::<Ctx>(&range));
        return Ok(None);
    }

    // Send request to peer
    let Some(request_id) = perform!(
        co,
        Effect::SendValueRequest(peer, ValueRequest::new(range.clone()), Default::default()),
        Resume::ValueRequestId(id) => id,
    ) else {
        warn!(range = %DisplayRange::<Ctx>(&range), %peer, "Failed to send sync request to peer");
        return Ok(None);
    };

    metrics.value_request_sent(range.start().as_u64());
    debug!(%request_id, range = %DisplayRange::<Ctx>(&range), %peer, "Sent sync request to peer");

    Ok(Some((request_id, range)))
}

/// Remove the pending request and re-request the batch from another peer.
/// If `except_peer_id` is provided, the request will be re-sent to a different peer than the one that sent the original request.
async fn re_request_values_from_peer_except<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    except_peer_id: Option<PeerId>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(%request_id, except_peer_id = ?except_peer_id, "Re-requesting values from peer");

    let Some((range, stored_peer_id)) = state.pending_requests.remove(&request_id.clone()) else {
        warn!(%request_id, "Unknown request ID when re-requesting values");
        return Ok(());
    };

    let except_peer_id = match except_peer_id {
        Some(peer_id) if stored_peer_id == peer_id => Some(peer_id),
        Some(peer_id) => {
            warn!(
                %request_id,
                peer.actual = %peer_id,
                peer.expected = %stored_peer_id,
                "Received response from different peer than expected"
            );

            Some(stored_peer_id)
        }
        None => None,
    };

    let Some((peer, peer_range)) = state.random_peer_with_except(&range, except_peer_id) else {
        debug!("No peer to re-request sync from");
        // Reset the sync height to the start of the range.
        state.sync_height = min(state.sync_height, *range.start());
        return Ok(());
    };

    // Send the request
    let Some((request_id, final_range)) =
        send_request_to_peer(&co, state, metrics, peer_range, peer).await?
    else {
        return Ok(()); // Request was skipped (empty range, etc.)
    };

    // Store the pending request (replacing the removed one)
    state
        .pending_requests
        .insert(request_id, (final_range.clone(), peer));

    Ok(())
}

struct DisplayRange<'a, Ctx: Context>(&'a RangeInclusive<Ctx::Height>);

impl<'a, Ctx: Context> core::fmt::Display for DisplayRange<'a, Ctx> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..={}", self.0.start(), self.0.end())
    }
}

/// Find the next uncovered range starting from initial_height.
///
/// Builds a contiguous range of the specified max_size from initial_height.
///
/// # Assumptions
/// - All ranges in pending_requests are disjoint (non-overlapping)
/// - initial_height is not covered by any pending request (maintained by caller)
///
/// # Panics
/// Panics if initial_height is already covered by a pending request (indicates a bug in the logic).
///
/// Returns the range that should be requested.
fn find_next_uncovered_range_from<Ctx>(
    initial_height: Ctx::Height,
    max_range_size: u64,
    pending_requests: &BTreeMap<OutboundRequestId, (RangeInclusive<Ctx::Height>, PeerId)>,
) -> RangeInclusive<Ctx::Height>
where
    Ctx: Context,
{
    let max_batch_size = max(1, max_range_size);

    // Find the pending request with the smallest range.start where range.end >= initial_height
    let next_range = pending_requests
        .values()
        .map(|(range, _)| range)
        .filter(|range| *range.end() >= initial_height)
        .min_by_key(|range| range.start());

    // Start with the full max_batch_size range
    let mut end_height = initial_height.increment_by(max_batch_size - 1);

    // If there's a range in pending, constrain to that boundary
    if let Some(range) = next_range {
        // Check if initial_height is covered by this earliest range
        if range.contains(&initial_height) {
            panic!(
                "Bug: initial_height {} is already covered by a pending request. This should never happen.",
                initial_height.as_u64()
            );
        }

        // Constrain to the blocking boundary
        let boundary_end = range
            .start()
            .decrement()
            .expect("range.start() should be decrementable since it's > initial_height");
        end_height = min(end_height, boundary_end);
    }

    initial_height..=end_height
}

/// Find the next height that's not covered by any pending request starting from starting_height.
fn find_next_uncovered_height<Ctx>(
    starting_height: Ctx::Height,
    pending_requests: &BTreeMap<OutboundRequestId, (RangeInclusive<Ctx::Height>, PeerId)>,
) -> Ctx::Height
where
    Ctx: Context,
{
    let mut next_height = starting_height;
    while let Some((covered_range, _)) = pending_requests
        .values()
        .find(|(r, _)| r.contains(&next_height))
    {
        next_height = covered_range.end().increment();
    }
    next_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use informalsystems_malachitebft_test::{Height, TestContext};
    use std::collections::BTreeMap;

    type TestPendingRequests = BTreeMap<OutboundRequestId, (RangeInclusive<Height>, PeerId)>;

    // Tests for the unified function find_next_uncovered_range_from_sync_height

    #[test]
    fn test_find_next_uncovered_range_from_no_pending_requests() {
        let pending_requests = TestPendingRequests::new();

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(10), 5, &pending_requests);

        assert_eq!(result, Height::new(10)..=Height::new(14));
    }

    #[test]
    fn test_find_next_uncovered_range_from_max_size_one() {
        let pending_requests = TestPendingRequests::new();

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(10), 1, &pending_requests);

        assert_eq!(result, Height::new(10)..=Height::new(10));
    }

    #[test]
    fn test_find_next_uncovered_range_from_with_blocking_request() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that blocks at height 12
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(12)..=Height::new(15), peer),
        );

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(10), 5, &pending_requests);

        // Should stop at height 11 because 12 is blocked
        assert_eq!(result, Height::new(10)..=Height::new(11));
    }

    #[test]
    fn test_find_next_uncovered_range_from_zero_max_size_becomes_one() {
        let pending_requests = TestPendingRequests::new();

        let result = find_next_uncovered_range_from::<TestContext>(
            Height::new(10),
            0, // Should be treated as 1
            &pending_requests,
        );

        assert_eq!(result, Height::new(10)..=Height::new(10));
    }

    #[test]
    fn test_find_next_uncovered_range_from_range_starts_immediately_after() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that starts immediately after initial_height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(16)..=Height::new(20), peer),
        );

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(15), 5, &pending_requests);

        // Should build only up to height 15 (boundary_end = 16 - 1 = 15, max_end would be 19)
        // min(19, 15) = 15
        assert_eq!(result, Height::new(15)..=Height::new(15));
    }

    #[test]
    fn test_find_next_uncovered_range_from_height_zero_with_range_starting_at_one() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers 1..=5
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(1)..=Height::new(5), peer),
        );

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(0), 3, &pending_requests);

        // Should build only height 0 (boundary_end = 1 - 1 = 0, max_end would be 2)
        // min(2, 0) = 0
        assert_eq!(result, Height::new(0)..=Height::new(0));
    }

    #[test]
    fn test_find_next_uncovered_range_from_sync_height_just_at_range_end() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request 5..=10, sync_height = 11
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(5)..=Height::new(10), peer),
        );

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(11), 4, &pending_requests);

        // Should build full range since no constraints (range 5..=10 has start=5 which is < 11)
        // max_end = 11 + 4 - 1 = 14
        assert_eq!(result, Height::new(11)..=Height::new(14));
    }

    #[test]
    fn test_find_next_uncovered_range_from_fill_gap_between_ranges() {
        let mut pending_requests = TestPendingRequests::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        // Add ranges before and after sync_height: 5..=10 and 20..=25, sync_height = 12
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(5)..=Height::new(10), peer1),
        );
        pending_requests.insert(
            OutboundRequestId::new("req2"),
            (Height::new(20)..=Height::new(25), peer2),
        );

        let result =
            find_next_uncovered_range_from::<TestContext>(Height::new(12), 6, &pending_requests);

        // Should fill gap up to range starting at 20
        // max_end = 12 + 6 - 1 = 17, boundary_end = 20 - 1 = 19
        // min(17, 19) = 17
        assert_eq!(result, Height::new(12)..=Height::new(17));
    }

    // Panic tests - initial_height is covered (indicates design bugs)

    #[test]
    #[should_panic(expected = "Bug: initial_height 12 is already covered by a pending request")]
    fn test_find_next_uncovered_range_from_sync_height_covered() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the sync_height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        // This should panic since initial_height 12 is covered by the range 10..=15
        find_next_uncovered_range_from::<TestContext>(Height::new(12), 3, &pending_requests);
    }

    #[test]
    #[should_panic(expected = "Bug: initial_height 15 is already covered by a pending request")]
    fn test_find_next_uncovered_range_from_initial_height_equals_range_start() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that starts exactly at initial_height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(15)..=Height::new(20), peer),
        );

        // This should panic since initial_height 15 equals range.start()
        find_next_uncovered_range_from::<TestContext>(Height::new(15), 5, &pending_requests);
    }

    #[test]
    #[should_panic(expected = "Bug: initial_height 15 is already covered by a pending request")]
    fn test_find_next_uncovered_range_from_sync_height_equals_range_end() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request 10..=15, sync_height = 15 (equals range end)
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        // This should panic since initial_height 15 is contained in range 10..=15 (inclusive)
        find_next_uncovered_range_from::<TestContext>(Height::new(15), 3, &pending_requests);
    }

    #[test]
    #[should_panic(expected = "Bug: initial_height 16 is already covered by a pending request")]
    fn test_find_next_uncovered_range_from_multiple_consecutive_blocks() {
        let mut pending_requests = TestPendingRequests::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        // Add consecutive pending requests
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer1),
        );
        pending_requests.insert(
            OutboundRequestId::new("req2"),
            (Height::new(16)..=Height::new(20), peer2),
        );

        // This should panic since initial_height 16 is covered by the range 16..=20
        find_next_uncovered_range_from::<TestContext>(Height::new(16), 3, &pending_requests);
    }

    #[test]
    #[should_panic(expected = "Bug: initial_height 0 is already covered by a pending request")]
    fn test_find_next_uncovered_range_from_sync_height_zero_with_range_starting_at_zero() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers 0..=5
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(0)..=Height::new(5), peer),
        );

        // This should panic since initial_height 0 is contained in range 0..=5
        find_next_uncovered_range_from::<TestContext>(Height::new(0), 3, &pending_requests);
    }

    // Tests for the helper function find_next_uncovered_height (used for sync_height updates)

    #[test]
    fn test_find_next_uncovered_height_no_pending_requests() {
        let pending_requests = TestPendingRequests::new();

        let result = find_next_uncovered_height::<TestContext>(Height::new(10), &pending_requests);

        assert_eq!(result, Height::new(10));
    }

    #[test]
    fn test_find_next_uncovered_height_starting_height_covered() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the starting height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(12), &pending_requests);

        // Should return the height after the covered range
        assert_eq!(result, Height::new(16));
    }

    #[test]
    fn test_find_next_uncovered_height_starting_height_match_request_start() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the starting height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(10), &pending_requests);

        // Should return the height after the covered range
        assert_eq!(result, Height::new(16));
    }

    #[test]
    fn test_find_next_uncovered_height_starting_height_match_request_end() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the starting height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(15), &pending_requests);

        // Should return the height after the covered range
        assert_eq!(result, Height::new(16));
    }

    #[test]
    fn test_find_next_uncovered_height_starting_height_just_before_request_start() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the starting height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(9), &pending_requests);

        // Should return the height after the covered range
        assert_eq!(result, Height::new(9));
    }

    #[test]
    fn test_find_next_uncovered_height_multiple_consecutive_ranges() {
        let mut pending_requests = TestPendingRequests::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        // Add consecutive pending requests
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer1),
        );
        pending_requests.insert(
            OutboundRequestId::new("req2"),
            (Height::new(16)..=Height::new(20), peer2),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(10), &pending_requests);

        // Should skip over all consecutive ranges
        assert_eq!(result, Height::new(21));
    }

    #[test]
    fn test_find_next_uncovered_height_multiple_consecutive_ranges_with_a_gap() {
        let mut pending_requests = TestPendingRequests::new();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        // Add consecutive pending requests
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer1),
        );
        pending_requests.insert(
            OutboundRequestId::new("req2"),
            (Height::new(16)..=Height::new(20), peer2),
        );
        pending_requests.insert(
            OutboundRequestId::new("req3"),
            (Height::new(24)..=Height::new(30), peer2),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(10), &pending_requests);

        // Should skip over all consecutive ranges
        assert_eq!(result, Height::new(21));
    }

    #[test]
    fn test_find_next_uncovered_height_starting_height_covered_nultiple() {
        let mut pending_requests = TestPendingRequests::new();
        let peer = PeerId::random();

        // Add a pending request that covers the starting height
        pending_requests.insert(
            OutboundRequestId::new("req1"),
            (Height::new(10)..=Height::new(15), peer),
        );
        pending_requests.insert(
            OutboundRequestId::new("req2"),
            (Height::new(15)..=Height::new(20), peer),
        );

        let result = find_next_uncovered_height::<TestContext>(Height::new(12), &pending_requests);

        // Should return the height after the covered range
        assert_eq!(result, Height::new(21));
    }
}

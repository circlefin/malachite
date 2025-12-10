use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use derive_where::derive_where;
use tracing::{debug, error, info, warn};

use malachitebft_core_types::utils::height::DisplayRange;
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
                        %request_id, actual_peer = %peer_id, expected_peer = %stored_peer_id,
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
                    warn!(
                        %request_id, %peer_id,
                        "Received request for wrong range of heights: expected {}..={} ({} values), got {}..={} ({} values)",
                        requested_range.start().as_u64(), requested_range.end().as_u64(), range_len,
                        start.as_u64(), end.as_u64(), response.values.len() as u64
                    );

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
    debug!(tip_height = %state.tip_height, "Broadcasting status");

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

    debug!("Peer scores: {:?}", state.peer_scorer.get_scores());

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

    debug!(%peer_id, %peer_height, "Received peer status");

    state.update_status(status);

    if !state.started {
        // Consensus has not started yet, no need to sync (yet).
        return Ok(());
    }

    if peer_height >= state.sync_height {
        info!(
            tip_height = %state.tip_height,
            sync_height = %state.sync_height,
            peer_height = %peer_height,
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
    debug!(%height, is_restart = %start_type.is_restart(), "Consensus started new height");

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

#[tracing::instrument(
    name = "on_value_request",
    skip_all,
    fields(
        peer_id = %peer_id,
        request_id = %request_id,
        range = %DisplayRange(&request.range)
    )
)]
pub async fn on_value_request<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    peer_id: PeerId,
    request: ValueRequest<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!("Received request for values");

    if !validate_request_range::<Ctx>(&request.range, state.tip_height, state.config.batch_size) {
        debug!("Sending empty response to peer");

        perform!(
            co,
            Effect::SendValueResponse(
                request_id.clone(),
                ValueResponse::new(*request.range.start(), vec![]),
                Default::default()
            )
        );

        return Ok(());
    }

    metrics.value_request_received(request.range.start().as_u64());

    let range = clamp_request_range::<Ctx>(&request.range, state.tip_height);

    if range != request.range {
        debug!(
            requested = %DisplayRange(&request.range),
            clamped = %DisplayRange(&range),
            "Clamped request range to our tip height"
        );
    }

    perform!(
        co,
        Effect::GetDecidedValues(request_id, range, Default::default())
    );

    Ok(())
}

fn validate_request_range<Ctx>(
    range: &RangeInclusive<Ctx::Height>,
    tip_height: Ctx::Height,
    batch_size: usize,
) -> bool
where
    Ctx: Context,
{
    if range.is_empty() {
        debug!("Received request for empty range of values");
        return false;
    }

    if range.start() > range.end() {
        debug!("Received request for invalid range of values");
        return false;
    }

    if range.start() > &tip_height {
        debug!("Received request for values beyond our tip height {tip_height}");
        return false;
    }

    let len = (range.end().as_u64() - range.start().as_u64()).saturating_add(1) as usize;
    if len > batch_size {
        warn!("Received request for too many values: requested {len}, max is {batch_size}");
        return false;
    }

    true
}

fn clamp_request_range<Ctx>(
    range: &RangeInclusive<Ctx::Height>,
    tip_height: Ctx::Height,
) -> RangeInclusive<Ctx::Height>
where
    Ctx: Context,
{
    assert!(!range.is_empty(), "Cannot clamp an empty range");
    assert!(
        *range.start() <= tip_height,
        "Cannot clamp range starting above tip height"
    );

    let start = *range.start();
    let end = min(*range.end(), tip_height);
    start..=end
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
            // Defensive check: This should never happen because this check is already performed in
            // the handler of `Input::ValueResponse`.
            error!(
                %request_id, peer.actual = %peer_id, peer.expected = %stored_peer_id,
                "Received response from different peer than expected"
            );
            return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
        }

        let range_len = requested_range.end().as_u64() - requested_range.start().as_u64() + 1;

        if response.values.len() < range_len as usize {
            // NOTE: We cannot simply call `re_request_values_from_peer_except` here.
            // Although we received some values from the peer, these values have not yet been processed
            // by the consensus engine. If we called `re_request_values_from_peer_except`, we would
            // end up re-requesting the entire original range (including values we already received),
            // causing the syncing peer to repeatedly send multiple requests until the already-received
            // values are fully processed.
            // To tackle this, we first update the current pending request with the range of values
            // it provides we received, and then issue a new request with the remaining values.
            let new_start = requested_range
                .start()
                .increment_by(response.values.len() as u64);

            let end = *requested_range.end();

            if response.values.is_empty() {
                error!(%request_id, %peer_id, "Received response contains no values");
            } else {
                // The response of this request only provided `response.values.len()` values,
                // so update the pending request accordingly
                let updated_range =
                    *requested_range.start()..=new_start.decrement().unwrap_or_default();

                state.update_request(request_id, peer_id, updated_range);
            }

            // Issue a new request to any peer, not necessarily the same one, for the remaining values
            let new_range = new_start..=end;
            request_values_range(co, state, metrics, new_range).await?;
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
    info!(%request_id, range = %DisplayRange(&range), "Received {} values from host", values.len());

    let start = range.start();
    let end = range.end();

    // Validate response from host
    let batch_size = end.as_u64() - start.as_u64() + 1;
    if batch_size != values.len() as u64 {
        warn!(
            %request_id,
            "Received {} values from host, expected {batch_size}",
            values.len()
        )
    }

    // Validate the height of each received value
    let mut height = *start;
    for value in &values {
        if value.certificate.height != height {
            error!(
                %request_id,
                "Received from host value for height {}, expected for height {height}",
                value.certificate.height
            );
        }
        height = height.increment();
    }

    debug!(%request_id, range = %DisplayRange(&range), "Sending response to peer");
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
            info!(%peer_id, range = %DisplayRange(&value_request.range), "Sync request timed out");

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
    let max_parallel_requests = state.max_parallel_requests();

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

/// Request values for this specific range from a peer.
/// Should only be used when re-requesting a partial range of values from a peer.
async fn request_values_range<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    range: RangeInclusive<Ctx::Height>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // NOTE: We do not perform a `max_parallel_requests` check and return here in contrast to what is done, for
    // example, in `request_values`. This is because `request_values_range` is only called for retrieving
    // partial responses, which means the original request is not on the wire anymore. Nevertheless,
    // we log here because seeing this log frequently implies that we keep getting partial responses
    // from peers and hints to potential reconfiguration.
    let max_parallel_requests = state.max_parallel_requests();
    if state.pending_requests.len() as u64 >= max_parallel_requests {
        info!(
            %max_parallel_requests,
            pending_requests = %state.pending_requests.len(),
            "Maximum number of pending requests reached when re-requesting a partial range of values"
        );
    };

    // Get a random peer that can provide the values in the range.
    let Some((peer, range)) = state.random_peer_with(&range) else {
        // No connected peer reached this height yet, we can stop syncing here.
        debug!(range = %DisplayRange(&range), "No peer to request sync from");
        return Ok(());
    };

    send_request_to_peer(&co, state, metrics, range, peer).await?;

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
    if range.is_empty() {
        debug!(%peer, "Range is empty, skipping request");
        return Ok(None);
    }

    // Skip over any heights in the range that are not waiting for a response
    // (meaning that they have been validated by consensus or a peer).
    let range = state.trim_validated_heights(&range);

    if range.is_empty() {
        warn!(
            range = %DisplayRange(&range), %peer,
            "All values in range have been validated, skipping request"
        );

        return Ok(None);
    }

    info!(range = %DisplayRange(&range), %peer, "Requesting sync from peer");

    // Send request to peer
    let Some(request_id) = perform!(
        co,
        Effect::SendValueRequest(peer, ValueRequest::new(range.clone()), Default::default()),
        Resume::ValueRequestId(id) => id,
    ) else {
        warn!(range = %DisplayRange(&range), %peer, "Failed to send sync request to peer");
        return Ok(None);
    };

    metrics.value_request_sent(range.start().as_u64());
    debug!(%request_id, range = %DisplayRange(&range), %peer, "Sent sync request to peer");

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

    // Test case structures for table-driven tests

    struct RangeTestCase {
        name: &'static str,
        initial_height: u64,
        max_size: u64,
        pending_ranges: &'static [(u64, u64)], // (start, end) pairs
        expected_start: u64,
        expected_end: u64,
    }

    struct PanicTestCase {
        name: &'static str,
        initial_height: u64,
        max_size: u64,
        pending_ranges: &'static [(u64, u64)], // (start, end) pairs
        expected_panic_msg: &'static str,
    }

    struct HeightTestCase {
        name: &'static str,
        initial_height: u64,
        pending_ranges: &'static [(u64, u64)], // (start, end) pairs
        expected_height: u64,
    }

    // Tests for find_next_uncovered_range_from function

    #[test]
    fn test_find_next_uncovered_range_from_table() {
        let test_cases = [
            RangeTestCase {
                name: "no pending requests",
                initial_height: 10,
                max_size: 5,
                pending_ranges: &[],
                expected_start: 10,
                expected_end: 14,
            },
            RangeTestCase {
                name: "max size one",
                initial_height: 10,
                max_size: 1,
                pending_ranges: &[],
                expected_start: 10,
                expected_end: 10,
            },
            RangeTestCase {
                name: "with blocking request",
                initial_height: 10,
                max_size: 5,
                pending_ranges: &[(12, 15)],
                expected_start: 10,
                expected_end: 11,
            },
            RangeTestCase {
                name: "zero max size becomes one",
                initial_height: 10,
                max_size: 0, // Should be treated as 1
                pending_ranges: &[],
                expected_start: 10,
                expected_end: 10,
            },
            RangeTestCase {
                name: "range starts immediately after",
                initial_height: 15,
                max_size: 5,
                pending_ranges: &[(16, 20)],
                expected_start: 15,
                expected_end: 15, // boundary_end = 16 - 1 = 15, min(19, 15) = 15
            },
            RangeTestCase {
                name: "height zero with range starting at one",
                initial_height: 0,
                max_size: 3,
                pending_ranges: &[(1, 5)],
                expected_start: 0,
                expected_end: 0, // boundary_end = 1 - 1 = 0, min(2, 0) = 0
            },
            RangeTestCase {
                name: "sync height just at range end",
                initial_height: 11,
                max_size: 4,
                pending_ranges: &[(5, 10)],
                expected_start: 11,
                expected_end: 14, // max_end = 11 + 4 - 1 = 14
            },
            RangeTestCase {
                name: "fill gap between ranges",
                initial_height: 12,
                max_size: 6,
                pending_ranges: &[(5, 10), (20, 25)],
                expected_start: 12,
                expected_end: 17, // max_end = 12 + 6 - 1 = 17, boundary_end = 20 - 1 = 19, min(17, 19) = 17
            },
        ];

        for case in test_cases {
            let mut pending_requests = TestPendingRequests::new();

            // Setup pending requests based on test case
            for (i, &(start, end)) in case.pending_ranges.iter().enumerate() {
                let peer = PeerId::random();
                pending_requests.insert(
                    OutboundRequestId::new(format!("req{}", i + 1)),
                    (Height::new(start)..=Height::new(end), peer),
                );
            }

            let result = find_next_uncovered_range_from::<TestContext>(
                Height::new(case.initial_height),
                case.max_size,
                &pending_requests,
            );

            assert_eq!(
                result,
                Height::new(case.expected_start)..=Height::new(case.expected_end),
                "Test case '{}' failed",
                case.name
            );
        }
    }

    // Panic tests for find_next_uncovered_range_from function

    #[test]
    fn test_find_next_uncovered_range_from_panic_cases() {
        let test_cases = [
            PanicTestCase {
                name: "sync height covered",
                initial_height: 12,
                max_size: 3,
                pending_ranges: &[(10, 15)],
                expected_panic_msg:
                    "Bug: initial_height 12 is already covered by a pending request",
            },
            PanicTestCase {
                name: "initial height equals range start",
                initial_height: 15,
                max_size: 5,
                pending_ranges: &[(15, 20)],
                expected_panic_msg:
                    "Bug: initial_height 15 is already covered by a pending request",
            },
            PanicTestCase {
                name: "sync height equals range end",
                initial_height: 15,
                max_size: 3,
                pending_ranges: &[(10, 15)],
                expected_panic_msg:
                    "Bug: initial_height 15 is already covered by a pending request",
            },
            PanicTestCase {
                name: "multiple consecutive blocks",
                initial_height: 16,
                max_size: 3,
                pending_ranges: &[(10, 15), (16, 20)],
                expected_panic_msg:
                    "Bug: initial_height 16 is already covered by a pending request",
            },
            PanicTestCase {
                name: "sync height zero with range starting at zero",
                initial_height: 0,
                max_size: 3,
                pending_ranges: &[(0, 5)],
                expected_panic_msg: "Bug: initial_height 0 is already covered by a pending request",
            },
        ];

        for case in test_cases {
            let mut pending_requests = TestPendingRequests::new();

            // Setup pending requests based on test case
            for (i, &(start, end)) in case.pending_ranges.iter().enumerate() {
                let peer = PeerId::random();
                pending_requests.insert(
                    OutboundRequestId::new(format!("req{}", i + 1)),
                    (Height::new(start)..=Height::new(end), peer),
                );
            }

            let result = std::panic::catch_unwind(|| {
                find_next_uncovered_range_from::<TestContext>(
                    Height::new(case.initial_height),
                    case.max_size,
                    &pending_requests,
                )
            });

            assert!(
                result.is_err(),
                "Test case '{}' should have panicked but didn't",
                case.name
            );

            if let Err(panic_value) = result {
                if let Some(panic_msg) = panic_value.downcast_ref::<String>() {
                    assert!(
                        panic_msg.contains(case.expected_panic_msg),
                        "Test case '{}' panicked with wrong message. Expected: '{}', Got: '{}'",
                        case.name,
                        case.expected_panic_msg,
                        panic_msg
                    );
                } else if let Some(panic_msg) = panic_value.downcast_ref::<&str>() {
                    assert!(
                        panic_msg.contains(case.expected_panic_msg),
                        "Test case '{}' panicked with wrong message. Expected: '{}', Got: '{}'",
                        case.name,
                        case.expected_panic_msg,
                        panic_msg
                    );
                }
            }
        }
    }

    // Tests for find_next_uncovered_height function

    #[test]
    fn test_find_next_uncovered_height_table() {
        let test_cases = [
            HeightTestCase {
                name: "no pending requests",
                initial_height: 10,
                pending_ranges: &[],
                expected_height: 10,
            },
            HeightTestCase {
                name: "starting height covered",
                initial_height: 12,
                pending_ranges: &[(10, 15)],
                expected_height: 16, // Should return the height after the covered range
            },
            HeightTestCase {
                name: "starting height match request start",
                initial_height: 10,
                pending_ranges: &[(10, 15)],
                expected_height: 16, // Should return the height after the covered range
            },
            HeightTestCase {
                name: "starting height match request end",
                initial_height: 15,
                pending_ranges: &[(10, 15)],
                expected_height: 16, // Should return the height after the covered range
            },
            HeightTestCase {
                name: "starting height just before request start",
                initial_height: 9,
                pending_ranges: &[(10, 15)],
                expected_height: 9, // Should return the starting height
            },
            HeightTestCase {
                name: "multiple consecutive ranges",
                initial_height: 10,
                pending_ranges: &[(10, 15), (16, 20)],
                expected_height: 21, // Should skip over all consecutive ranges
            },
            HeightTestCase {
                name: "multiple consecutive ranges with a gap",
                initial_height: 10,
                pending_ranges: &[(10, 15), (16, 20), (24, 30)],
                expected_height: 21, // Should skip over consecutive ranges but stop at gap
            },
            HeightTestCase {
                name: "starting height covered multiple",
                initial_height: 12,
                pending_ranges: &[(10, 15), (15, 20)],
                expected_height: 21, // Should return the height after all covered ranges
            },
        ];

        for case in test_cases {
            let mut pending_requests = TestPendingRequests::new();

            // Setup pending requests based on test case
            for (i, &(start, end)) in case.pending_ranges.iter().enumerate() {
                let peer = PeerId::random();
                pending_requests.insert(
                    OutboundRequestId::new(format!("req{}", i + 1)),
                    (Height::new(start)..=Height::new(end), peer),
                );
            }

            let result = find_next_uncovered_height::<TestContext>(
                Height::new(case.initial_height),
                &pending_requests,
            );

            assert_eq!(
                result,
                Height::new(case.expected_height),
                "Test case '{}' failed",
                case.name
            );
        }
    }

    #[test]
    fn test_validate_request_range() {
        let validate = validate_request_range::<TestContext>;

        let tip_height = Height::new(20);
        let batch_size = 5;

        // Valid range
        let range = Height::new(15)..=Height::new(19);
        assert!(validate(&range, tip_height, batch_size));

        // Start greater than end
        let range = Height::new(18)..=Height::new(17);
        assert!(!validate(&range, tip_height, batch_size));

        // Start greater than tip height
        let range = Height::new(21)..=Height::new(25);
        assert!(!validate(&range, tip_height, batch_size));

        // Exceeds batch size
        let range = Height::new(10)..=Height::new(16);
        assert!(!validate(&range, tip_height, batch_size));

        // No overflow
        let range = Height::new(0)..=Height::new(u64::MAX);
        assert!(!validate(&range, tip_height, batch_size));
    }

    #[test]
    fn test_clamp_request_range() {
        let clamp = clamp_request_range::<TestContext>;

        let tip_height = Height::new(20);

        // Range within tip height
        let range = Height::new(15)..=Height::new(18);
        let clamped = clamp(&range, tip_height);
        assert_eq!(clamped, range);

        // Range exceeding tip height
        let range = Height::new(18)..=Height::new(25);
        let clamped = clamp(&range, tip_height);
        assert_eq!(clamped, Height::new(18)..=tip_height);

        // Range starting at tip height
        let range = tip_height..=Height::new(25);
        let clamped = clamp(&range, tip_height);
        assert_eq!(clamped, tip_height..=tip_height);
    }
}

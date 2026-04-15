use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use derive_where::derive_where;
use tracing::{debug, error, info, warn};

use malachitebft_core_types::utils::height::{DisplayRange, HeightRangeExt};
use malachitebft_core_types::{Context, Height};

use crate::co::Co;
use crate::scoring::SyncResult;
use crate::{
    perform, Effect, Error, HeightStartType, InboundRequestId, Metrics, OutboundRequestId, PeerId,
    PendingRequestEntry, RawDecidedValue, Request, Resume, State, Status, ValueRequest,
    ValueResponse,
};

#[derive_where(Debug)]
pub enum Input<Ctx: Context> {
    /// Periodical event triggering the broadcast of a status update
    SendStatusUpdate,

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
        Input::SendStatusUpdate => on_send_status_update(co, state, metrics).await,

        Input::Status(status) => on_status(co, state, metrics, status).await,

        Input::StartedHeight(height, restart) => {
            on_started_height(co, state, metrics, height, restart).await
        }

        Input::Decided(height) => on_decided(state, metrics, height).await,

        Input::ValueRequest(request_id, peer_id, request) => {
            on_value_request(co, state, metrics, request_id, peer_id, request).await
        }

        Input::ValueResponse(request_id, peer_id, Some(response)) => {
            on_value_response(co, state, metrics, request_id, peer_id, response).await
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

async fn on_value_response<Ctx>(
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
    let end = response.end_height().unwrap_or(start);
    let range_len = end.as_u64() - start.as_u64() + 1;

    // Check if the response is valid. A valid response starts at the
    // requested start height, has at least one value, and no more than
    // the requested range.
    let Some(entry) = state.pending_requests.get(&request_id) else {
        warn!(%request_id, %peer_id, "Received response for unknown request ID");
        return Ok(());
    };
    let requested_range = &entry.range;
    let stored_peer_id = &entry.peer;

    if stored_peer_id != &peer_id {
        warn!(
            %request_id, actual_peer = %peer_id, expected_peer = %stored_peer_id,
            "Received response from different peer than expected"
        );

        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    let range_valid = start.as_u64() == requested_range.start().as_u64()
        && start.as_u64() <= end.as_u64()
        && end.as_u64() <= requested_range.end().as_u64()
        && response.values.len() as u64 == range_len;

    if !range_valid {
        warn!(
            %request_id, %peer_id,
            "Received response with wrong range: expected {}..={} ({} values), got {}..={} ({} values)",
            requested_range.start().as_u64(), requested_range.end().as_u64(), range_len,
            start.as_u64(), end.as_u64(), response.values.len() as u64
        );

        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    // Verify that each value's certificate height matches its expected sequential position
    let heights_sequential = response
        .values
        .iter()
        .enumerate()
        .all(|(i, value)| value.height().as_u64() == start.as_u64() + i as u64);

    if !heights_sequential {
        warn!(
            %request_id, %peer_id,
            "Received response with non-sequential certificate heights for range {}..={}",
            start.as_u64(), end.as_u64(),
        );

        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    if !validate_value_response_heights::<Ctx>(&response) {
        warn!(
            %request_id, %peer_id,
            "Response contains non-contiguous heights"
        );

        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    on_valid_value_response(co, state, metrics, request_id, peer_id, response).await
}

/// Validate that each value in the response has the expected height,
/// ie. heights are contiguous starting from `start_height`.
fn validate_value_response_heights<Ctx>(response: &ValueResponse<Ctx>) -> bool
where
    Ctx: Context,
{
    response.values.iter().enumerate().all(|(i, value)| {
        let expected = response.start_height.increment_by(i as u64);
        value.height() == expected
    })
}

pub async fn on_send_status_update<Ctx>(
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
    metrics.status_received(state.peers.len() as u64);

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
    state.consensus_height = height;

    // The tip is the last decided value.
    state.tip_height = height.decrement().unwrap_or_default();

    // Garbage collect fully-validated requests.
    state.prune_pending_requests();

    if start_type.is_restart() {
        // Consensus is retrying the height, so we should sync starting from it.
        // Clear pending requests, as we are restarting the height.
        state.pending_requests.clear();
        set_sync_height(state, height);
    } else {
        // If consensus is voting on a height that is currently being synced from a peer, do not update the sync height.
        set_sync_height(state, max(state.sync_height, height));
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

    // Garbage collect pending requests for heights up to the new tip.
    state.prune_pending_requests();

    // Re-validate sync_height after tip advanced.
    set_sync_height(state, state.sync_height);

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

pub async fn on_valid_value_response<Ctx>(
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

    let values_count = response.values.len();

    // Tell consensus to process the response.
    perform!(
        co,
        Effect::ProcessValueResponse(peer_id, request_id.clone(), response, Default::default())
    );

    // If the response contains a prefix of the requested values, re-request the remaining values.
    //
    // Extract cheap Copy data from the entry. NLL releases the immutable borrow
    // after the last use of `entry`, so mutable access to `state` is available
    // below. The entry stays in the map and is only removed in the single path
    // that needs to replace it (partial response with >0 values).
    let Some(entry) = state.pending_requests.get(&request_id) else {
        return Ok(());
    };

    let (entry_peer, entry_range) = (entry.peer, &entry.range);
    let range_start = *entry_range.start();
    let range_end = *entry_range.end();
    let range_len = entry_range.len();

    if entry_peer != peer_id {
        // Defensive check: This should never happen because this check is already performed in
        // the handler of `Input::ValueResponse`.
        error!(
            %request_id, peer.actual = %peer_id, peer.expected = %entry_peer,
            "Received response from different peer than expected"
        );

        // Entry is still in the map — on_invalid_value_response will remove it.
        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    if values_count < range_len {
        // NOTE: We cannot simply call `re_request_values_from_peer_except` here.
        // Although we received some values from the peer, these values have not yet been processed
        // by the consensus engine. If we called `re_request_values_from_peer_except`, we would
        // end up re-requesting the entire original range (including values we already received),
        // causing the syncing peer to repeatedly send multiple requests until the already-received
        // values are fully processed.
        // To tackle this, we first update the current pending request with the range of values
        // it provides we received, and then issue a new request with the remaining values.
        let new_start = range_start.increment_by(values_count as u64);

        if values_count == 0 {
            error!(%request_id, %peer_id, "Received response contains no values");
            // Entry stays unchanged in the map — nothing to do.
        } else {
            // Only path that needs to modify the entry — remove to take ownership of excluded_peers.
            let entry = state.pending_requests.remove(&request_id).unwrap();
            let updated_range = range_start..=new_start.decrement().unwrap_or_default();
            state.update_request(request_id, peer_id, updated_range, entry.excluded_peers);
        }

        // Issue a new request to any peer, not necessarily the same one, for the remaining values
        let new_range = new_start..=range_end;
        request_values_range(co, state, metrics, new_range).await?;
    }
    // Full response — entry stays as-is; prune_pending_requests cleans it up
    // once consensus advances past this range.

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
    mut values: Vec<RawDecidedValue<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(%request_id, range = %DisplayRange(&range), "Received {} values from host", values.len());

    let start = range.start();
    let end = range.end();

    // Log if host returned a different number of values than expected.
    // This can happen legitimately (e.g. truncation due to response size limits)
    // so we only warn but do not reject the response.
    let batch_size = end.as_u64() - start.as_u64() + 1;
    if batch_size != values.len() as u64 {
        warn!(
            %request_id,
            "Received {} values from host, expected {batch_size}",
            values.len()
        );
    }

    // Validate the height of each received value.
    // Truncate at the first value with an unexpected height and forward
    // the valid contiguous prefix so the requesting peer can still use it.
    let mut height = *start;
    let mut valid_count = 0;
    for value in &values {
        if value.certificate.height != height {
            error!(
                %request_id,
                "Received from host value for height {}, expected height: {height}; \
                 sending {valid_count} valid values to peer",
                value.certificate.height
            );
            break;
        }
        valid_count += 1;
        height = height.increment();
    }

    values.truncate(valid_count);

    debug!(%request_id, range = %DisplayRange(&range), "Sending {} values to peer", values.len());
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

    if state.pending_requests.len() >= max_parallel_requests {
        info!(
            max_parallel_requests,
            pending_requests = state.pending_requests.len(),
            "Maximum number of parallel requests reached, skipping request for values"
        );

        return Ok(());
    };

    while state.pending_requests.len() < max_parallel_requests {
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

        send_and_track_request_to_peer(&co, state, metrics, peer, range, BTreeSet::new()).await?;
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

    if state.pending_requests.len() >= max_parallel_requests {
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

    send_and_track_request_to_peer(&co, state, metrics, peer, range, BTreeSet::new()).await?;

    Ok(())
}

async fn send_and_track_request_to_peer<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer: PeerId,
    range: RangeInclusive<<Ctx as Context>::Height>,
    excluded_peers: BTreeSet<PeerId>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // Send the request
    let Some((request_id, final_range)) =
        send_request_to_peer(co, state, metrics, range, peer).await?
    else {
        return Ok(()); // Request was skipped (empty range, etc.)
    };

    // Store the pending request
    state.pending_requests.insert(
        request_id,
        PendingRequestEntry {
            range: final_range.clone(),
            peer,
            excluded_peers,
        },
    );

    // Update sync_height to the next uncovered height after this range
    set_sync_height(state, final_range.end().increment());

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
///
/// If `except_peer_id` is `Some`, the failed peer is added to the set of
/// excluded peers accumulated across retries. Once every eligible peer has
/// been tried and failed, no further retry is attempted and sync_height is
/// reset so a future event (status update, consensus advance) can restart
/// the request cycle with a clean slate.
///
/// If `except_peer_id` is `None` (internal processing error), no peer is
/// added to the exclusion set because the failure was not the peer's fault.
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

    let Some(mut entry) = state.pending_requests.remove(&request_id) else {
        warn!(%request_id, "Unknown request ID when re-requesting values");
        return Ok(());
    };

    match except_peer_id {
        Some(peer_id) if entry.peer == peer_id => {
            entry.excluded_peers.insert(peer_id);
        }
        Some(peer_id) => {
            warn!(
                %request_id,
                peer.actual = %peer_id,
                peer.expected = %entry.peer,
                "Received response from different peer than expected"
            );

            entry.excluded_peers.insert(entry.peer);
            entry.excluded_peers.insert(peer_id);
        }
        None => {
            // Internal processing error — not the peer's fault, don't exclude anyone.
        }
    };

    let Some((peer, peer_range)) =
        state.random_peer_with_except(&entry.range, &entry.excluded_peers)
    else {
        debug!(
            excluded_peers = entry.excluded_peers.len(),
            "No peer to re-request sync from, all eligible peers exhausted"
        );
        // Reset sync_height towards the start of the failed range so it can be retried
        // when conditions change (new status update, consensus advance, peer reconnect).
        set_sync_height(state, min(state.sync_height, *entry.range.start()));
        return Ok(());
    };

    send_and_track_request_to_peer(&co, state, metrics, peer, peer_range, entry.excluded_peers)
        .await?;

    Ok(())
}

/// Set `sync_height` to the given candidate while enforcing both invariants:
///   - `sync_height > tip_height`
///   - `sync_height` is not covered by any pending request
///
/// If the candidate violates either invariant, it is raised to the next
/// uncovered height at or above `tip_height + 1`.
fn set_sync_height<Ctx: Context>(state: &mut State<Ctx>, candidate: Ctx::Height) {
    let floor = max(state.tip_height.increment(), candidate);
    let new_sync_height = find_next_uncovered_height::<Ctx>(floor, &state.pending_requests);

    if new_sync_height != candidate {
        warn!(
            %candidate,
            tip_height = %state.tip_height,
            sync_height = %new_sync_height,
            "Adjusted sync_height from candidate to satisfy invariants"
        );
    }

    state.sync_height = new_sync_height;
}

/// Find the next uncovered range starting from initial_height.
///
/// Builds a contiguous range of the specified max_size from initial_height.
///
/// # Assumptions
/// - All ranges in pending_requests are disjoint (non-overlapping)
/// - initial_height is not covered by any pending request (maintained by caller via `set_sync_height`)
///
/// If initial_height is unexpectedly covered by a pending request, the function recovers
/// by advancing to the first uncovered height after the conflicting range.
///
/// Returns the range that should be requested.
fn find_next_uncovered_range_from<Ctx>(
    mut initial_height: Ctx::Height,
    max_range_size: u64,
    pending_requests: &BTreeMap<OutboundRequestId, PendingRequestEntry<Ctx::Height>>,
) -> RangeInclusive<Ctx::Height>
where
    Ctx: Context,
{
    let max_batch_size = max(1, max_range_size);

    // If initial_height is inside a pending request, recover by advancing past it.
    // This should not happen if all sync_height writes go through set_sync_height.
    let adjusted = find_next_uncovered_height::<Ctx>(initial_height, pending_requests);
    if adjusted != initial_height {
        error!(
            initial_height = %initial_height.as_u64(),
            adjusted_height = %adjusted.as_u64(),
            "initial_height was inside a pending request, advancing past it"
        );
        initial_height = adjusted;
    }

    // Find the pending request with the smallest range.start where range.end >= initial_height
    let next_range = pending_requests
        .values()
        .map(|entry| &entry.range)
        .filter(|range| *range.end() >= initial_height)
        .min_by_key(|range| range.start());

    // Start with the full max_batch_size range
    let mut end_height = initial_height.increment_by(max_batch_size - 1);

    // If there's a range in pending, constrain to that boundary
    if let Some(range) = next_range {
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
    pending_requests: &BTreeMap<OutboundRequestId, PendingRequestEntry<Ctx::Height>>,
) -> Ctx::Height
where
    Ctx: Context,
{
    let mut next_height = starting_height;
    while let Some(entry) = pending_requests
        .values()
        .find(|entry| entry.range.contains(&next_height))
    {
        next_height = entry.range.end().increment();
    }
    next_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_malachitebft_test::{Height, TestContext, ValueId};
    use bytes::Bytes;
    use malachitebft_core_types::{CommitCertificate, Round};
    use rand::SeedableRng;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::effect::Resumable;
    use crate::Config;

    type TestPendingRequests = BTreeMap<OutboundRequestId, PendingRequestEntry<Height>>;

    // Test case structures for table-driven tests

    struct RangeTestCase {
        name: &'static str,
        initial_height: u64,
        max_size: u64,
        pending_ranges: &'static [(u64, u64)], // (start, end) pairs
        expected_start: u64,
        expected_end: u64,
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
                    PendingRequestEntry {
                        range: Height::new(start)..=Height::new(end),
                        peer,
                        excluded_peers: BTreeSet::new(),
                    },
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

    // Recovery tests for find_next_uncovered_range_from: when initial_height
    // falls inside a pending request, the function skips past it.

    #[test]
    fn test_find_next_uncovered_range_from_recovery_cases() {
        let test_cases = [
            RangeTestCase {
                name: "initial height inside pending range, recovers past it",
                initial_height: 12,
                max_size: 3,
                pending_ranges: &[(10, 15)],
                expected_start: 16,
                expected_end: 18,
            },
            RangeTestCase {
                name: "initial height equals range start, recovers past it",
                initial_height: 15,
                max_size: 5,
                pending_ranges: &[(15, 20)],
                expected_start: 21,
                expected_end: 25,
            },
            RangeTestCase {
                name: "initial height equals range end, recovers past it",
                initial_height: 15,
                max_size: 3,
                pending_ranges: &[(10, 15)],
                expected_start: 16,
                expected_end: 18,
            },
            RangeTestCase {
                name: "multiple consecutive ranges, recovers past all",
                initial_height: 16,
                max_size: 3,
                pending_ranges: &[(10, 15), (16, 20)],
                expected_start: 21,
                expected_end: 23,
            },
            RangeTestCase {
                name: "initial height zero inside range starting at zero",
                initial_height: 0,
                max_size: 3,
                pending_ranges: &[(0, 5)],
                expected_start: 6,
                expected_end: 8,
            },
        ];

        for case in test_cases {
            let mut pending_requests = TestPendingRequests::new();

            for (i, &(start, end)) in case.pending_ranges.iter().enumerate() {
                let peer = PeerId::random();
                pending_requests.insert(
                    OutboundRequestId::new(format!("req{}", i + 1)),
                    PendingRequestEntry {
                        range: Height::new(start)..=Height::new(end),
                        peer,
                        excluded_peers: BTreeSet::new(),
                    },
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
                    PendingRequestEntry {
                        range: Height::new(start)..=Height::new(end),
                        peer,
                        excluded_peers: BTreeSet::new(),
                    },
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

    #[test]
    fn test_validate_value_response_heights() {
        let validate = validate_value_response_heights::<TestContext>;

        // Valid: contiguous heights 5, 6, 7
        let response = ValueResponse::new(
            Height::new(5),
            vec![
                make_raw_decided_value(5),
                make_raw_decided_value(6),
                make_raw_decided_value(7),
            ],
        );
        assert!(validate(&response));

        // Valid: single value
        let response = ValueResponse::new(Height::new(1), vec![make_raw_decided_value(1)]);
        assert!(validate(&response));

        // Valid: empty response
        let response = ValueResponse::new(Height::new(1), vec![]);
        assert!(validate(&response));

        // Invalid: gap in heights (1, 2, 5 instead of 1, 2, 3)
        let response = ValueResponse::new(
            Height::new(1),
            vec![
                make_raw_decided_value(1),
                make_raw_decided_value(2),
                make_raw_decided_value(5),
            ],
        );
        assert!(!validate(&response));

        // Invalid: duplicate heights (1, 1, 2 instead of 1, 2, 3)
        let response = ValueResponse::new(
            Height::new(1),
            vec![
                make_raw_decided_value(1),
                make_raw_decided_value(1),
                make_raw_decided_value(2),
            ],
        );
        assert!(!validate(&response));

        // Invalid: first value doesn't match start_height
        let response = ValueResponse::new(
            Height::new(1),
            vec![
                make_raw_decided_value(2),
                make_raw_decided_value(3),
                make_raw_decided_value(4),
            ],
        );
        assert!(!validate(&response));

        // Invalid: reversed order (3, 2, 1 instead of 1, 2, 3)
        let response = ValueResponse::new(
            Height::new(1),
            vec![
                make_raw_decided_value(3),
                make_raw_decided_value(2),
                make_raw_decided_value(1),
            ],
        );
        assert!(!validate(&response));
    }

    fn make_raw_decided_value(height: u64) -> RawDecidedValue<TestContext> {
        RawDecidedValue {
            value_bytes: Bytes::new(),
            certificate: CommitCertificate {
                height: Height::new(height),
                round: Round::new(0),
                value_id: ValueId::new(height),
                commit_signatures: vec![],
            },
        }
    }

    /// Test that a non-contiguous sync response (e.g., request 1..=10, get 1,2,5..12)
    /// is rejected by the sync state machine and triggers a re-request from another peer.
    #[test]
    fn test_non_contiguous_response_rejected_by_sync_handler() {
        use std::cell::Cell;

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();
        let request_id = OutboundRequestId::new("req-1");

        let mut state = State::<TestContext>::new(
            Box::new(rand::rngs::StdRng::seed_from_u64(42)),
            Config::default(),
        );

        // Set up state: consensus is at height 1, pending request for 1..=10 from peer_a
        state.consensus_height = Height::new(1);
        state.tip_height = Height::new(0);
        state.sync_height = Height::new(11);
        state.started = true;
        state.pending_requests.insert(
            request_id.clone(),
            PendingRequestEntry {
                range: Height::new(1)..=Height::new(10),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
            },
        );

        // Add peer_b so re-request can find another peer
        state.update_status(Status {
            peer_id: peer_b,
            tip_height: Height::new(20),
            history_min_height: Height::new(1),
        });

        // Build a malformed response: 10 values starting at height 1
        // but with a gap (heights 1, 2, 5, 6, 7, 8, 9, 10, 11, 12)
        let response = ValueResponse::new(
            Height::new(1),
            vec![
                make_raw_decided_value(1),
                make_raw_decided_value(2),
                make_raw_decided_value(5),
                make_raw_decided_value(6),
                make_raw_decided_value(7),
                make_raw_decided_value(8),
                make_raw_decided_value(9),
                make_raw_decided_value(10),
                make_raw_decided_value(11),
                make_raw_decided_value(12),
            ],
        );

        let input = Input::ValueResponse(request_id, peer_a, Some(response));
        let metrics = Metrics::default();

        // The handler should reject the response and re-request from another peer.
        // It should yield SendValueRequest (to peer_b), NOT ProcessValueResponse.
        let saw_send_request = Cell::new(false);
        let saw_process_response = Cell::new(false);

        let result: Result<(), Error<TestContext>> = (|| {
            crate::process!(
                input: input,
                state: &mut state,
                metrics: &metrics,
                with: effect => {
                    match &effect {
                        Effect::SendValueRequest(peer, _, _) => {
                            saw_send_request.set(true);
                            assert_eq!(*peer, peer_b);
                        }
                        Effect::ProcessValueResponse(_, _, _, _) => {
                            saw_process_response.set(true);
                        }
                        _ => {}
                    }

                    Ok::<_, eyre::Report>(match effect {
                        Effect::SendValueRequest(_, _, r) => {
                            r.resume_with(Some(OutboundRequestId::new("req-2")))
                        }
                        Effect::BroadcastStatus(_, r) => r.resume_with(()),
                        Effect::SendValueResponse(_, _, r) => r.resume_with(()),
                        Effect::GetDecidedValues(_, _, r) => r.resume_with(()),
                        Effect::ProcessValueResponse(_, _, _, r) => r.resume_with(()),
                    })
                }
            )
        })();

        assert!(result.is_ok(), "Handler returned error: {result:?}");
        assert!(
            saw_send_request.get(),
            "Expected a re-request to another peer after non-contiguous response"
        );
        assert!(
            !saw_process_response.get(),
            "Non-contiguous response should NOT have been forwarded to consensus"
        );
    }
    // Helper: drive a handle::Input through the coroutine-based handler.
    // Collects all yielded effects and auto-resumes with default values.
    // Only works for inputs whose handling does not require meaningful resume values
    // (ie. the no-peer / no-yield paths we are testing here).
    fn drive_input(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
    ) -> Result<Vec<crate::Effect<TestContext>>, crate::Error<TestContext>> {
        use crate::co::{CoState, Gen};
        use crate::Resume;

        let mut effects = Vec::new();
        let mut gen = Gen::new(|co| handle(co, state, metrics, input));
        let mut result = gen.resume_with(Resume::default());

        loop {
            match result {
                CoState::Yielded(effect) => {
                    effects.push(effect);
                    result = gen.resume_with(Resume::default());
                }
                CoState::Complete(r) => return r.map(|()| effects),
            }
        }
    }

    fn make_test_state() -> State<TestContext> {
        use rand::SeedableRng;
        State::new(
            Box::new(rand::rngs::StdRng::seed_from_u64(42)),
            crate::Config::default(),
        )
    }

    // -------------------------------------------------------------------
    // sync_height invariants:
    //   1. sync_height > tip_height
    //   2. sync_height must not fall inside any pending request's range
    // -------------------------------------------------------------------

    // -- on_decided: sync_height must advance past tip_height --

    #[test]
    fn test_on_decided_advances_sync_height_when_equal_to_new_tip() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(9);
        state.sync_height = Height::new(10);

        drive_input(&mut state, &metrics, Input::Decided(Height::new(10))).unwrap();

        assert_eq!(state.tip_height, Height::new(10));
        assert_eq!(state.sync_height, Height::new(11));
    }

    #[test]
    fn test_on_decided_advances_sync_height_when_below_new_tip() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(9);
        state.sync_height = Height::new(8);

        drive_input(&mut state, &metrics, Input::Decided(Height::new(10))).unwrap();

        assert_eq!(state.tip_height, Height::new(10));
        assert_eq!(state.sync_height, Height::new(11));
        assert!(state.sync_height > state.tip_height);
    }

    #[test]
    fn test_on_decided_preserves_sync_height_when_already_ahead() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(9);
        state.sync_height = Height::new(20);

        drive_input(&mut state, &metrics, Input::Decided(Height::new(10))).unwrap();

        assert_eq!(state.tip_height, Height::new(10));
        assert_eq!(state.sync_height, Height::new(20));
    }

    #[test]
    fn test_on_decided_skips_pending_requests() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(109);
        state.sync_height = Height::new(110);

        let peer_a = PeerId::random();
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(110)..=Height::new(120),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
            },
        );

        // Deciding heights 110..=112 advances tip to 112.
        // sync_height must not land inside the remaining pending request [110..=120].
        drive_input(&mut state, &metrics, Input::Decided(Height::new(110))).unwrap();
        drive_input(&mut state, &metrics, Input::Decided(Height::new(111))).unwrap();
        drive_input(&mut state, &metrics, Input::Decided(Height::new(112))).unwrap();

        assert_eq!(state.tip_height, Height::new(112));
        for entry in state.pending_requests.values() {
            let range = &entry.range;
            assert!(
                !range.contains(&state.sync_height),
                "sync_height ({}) inside pending request range {}..={}",
                state.sync_height.as_u64(),
                range.start().as_u64(),
                range.end().as_u64(),
            );
        }
    }

    // -- on_started_height: sync_height must skip pending requests --

    #[test]
    fn test_on_started_height_skips_pending_requests() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(99);
        state.sync_height = Height::new(121);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(100)..=Height::new(110),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req2"),
            PendingRequestEntry {
                range: Height::new(111)..=Height::new(120),
                peer: peer_b,
                excluded_peers: BTreeSet::new(),
            },
        );
        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(120),
                history_min_height: Height::new(1),
            },
        );

        // req1 times out, no alternative peer → sync_height resets.
        drive_input(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req1"),
                peer_a,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(100)..=Height::new(110),
                )),
            ),
        )
        .unwrap();

        // Consensus advances to 115.
        for h in 100..=114 {
            drive_input(&mut state, &metrics, Input::Decided(Height::new(h))).unwrap();
        }

        // on_started_height(115) must not place sync_height inside [111..=120].
        drive_input(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(115), HeightStartType::Start),
        )
        .unwrap();

        for entry in state.pending_requests.values() {
            let range = &entry.range;
            assert!(
                !range.contains(&state.sync_height),
                "sync_height ({}) inside pending request range {}..={}",
                state.sync_height.as_u64(),
                range.start().as_u64(),
                range.end().as_u64(),
            );
        }
    }

    #[test]
    fn test_on_started_height_restart_clears_pending_and_resets() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(9);
        state.sync_height = Height::new(15);
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(10)..=Height::new(14),
                peer: PeerId::random(),
                excluded_peers: BTreeSet::new(),
            },
        );

        drive_input(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(10), HeightStartType::Restart),
        )
        .unwrap();

        assert_eq!(state.sync_height, Height::new(10));
        assert!(state.pending_requests.is_empty());
    }

    // -- re_request_values_from_peer_except: sync_height invariants --

    #[test]
    fn test_re_request_no_peer_preserves_sync_height_above_tip() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        // Pending request for 11..=15, sync_height = 16.
        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
            },
        );
        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(15),
                history_min_height: Height::new(1),
            },
        );

        // Consensus decides 11 and 12 while the request is in flight.
        drive_input(&mut state, &metrics, Input::Decided(Height::new(11))).unwrap();
        drive_input(&mut state, &metrics, Input::Decided(Height::new(12))).unwrap();

        assert_eq!(state.tip_height, Height::new(12));

        // Request times out, no alternative peer.
        // sync_height must remain above tip_height.
        drive_input(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req1"),
                peer_a,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
            ),
        )
        .unwrap();

        assert!(
            state.sync_height > state.tip_height,
            "sync_height ({}) <= tip_height ({})",
            state.sync_height.as_u64(),
            state.tip_height.as_u64(),
        );
        assert_eq!(state.sync_height, Height::new(13));
    }

    // -- re_request: excluded peers accumulate across retries --

    /// Like [`drive_input`] but provides `ValueRequestId` resumes when
    /// `SendValueRequest` effects are yielded, allowing retry paths to
    /// complete without error.
    fn drive_input_with_retries(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
    ) -> Result<Vec<crate::Effect<TestContext>>, crate::Error<TestContext>> {
        use crate::co::{CoState, Gen};
        use crate::Resume;

        let mut effects = Vec::new();
        let mut gen = Gen::new(|co| handle(co, state, metrics, input));
        let mut result = gen.resume_with(Resume::default());
        let mut req_counter = 0u64;

        loop {
            match result {
                CoState::Yielded(effect) => {
                    let resume = match &effect {
                        Effect::SendValueRequest(..) => {
                            req_counter += 1;
                            Resume::ValueRequestId(Some(OutboundRequestId::new(format!(
                                "retry_req{req_counter}"
                            ))))
                        }
                        _ => Resume::default(),
                    };
                    effects.push(effect);
                    result = gen.resume_with(resume);
                }
                CoState::Complete(r) => return r.map(|()| effects),
            }
        }
    }

    #[test]
    fn test_re_request_stops_after_all_peers_exhausted() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        // Register both peers as having the data.
        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );
        state.peers.insert(
            peer_b,
            crate::Status {
                peer_id: peer_b,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );

        // Pending request assigned to peer_a for heights 11..=15.
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
            },
        );

        // Peer A times out — retry should go to peer B with A in the excluded set.
        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req1"),
                peer_a,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
            ),
        )
        .unwrap();

        // A new request should have been sent (to peer B).
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "Expected a new request after peer A timed out"
        );

        // Verify the new pending request carries A in the excluded set.
        assert_eq!(state.pending_requests.len(), 1);
        let (new_req_id, entry) = state.pending_requests.iter().next().unwrap();
        assert_ne!(entry.peer, peer_a, "Retry should not go back to peer A");
        assert!(
            entry.excluded_peers.contains(&peer_a),
            "Peer A should be in the excluded set"
        );
        let new_req_id = new_req_id.clone();

        // Peer B also times out — all peers exhausted, no further retry.
        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                new_req_id,
                peer_b,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
            ),
        )
        .unwrap();

        // No new request should be sent.
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "No request should be sent after all peers are exhausted"
        );

        // Pending requests should be empty.
        assert!(
            state.pending_requests.is_empty(),
            "No pending requests should remain"
        );

        // sync_height should have been reset but remain above tip_height.
        // sync_height should reset to the start of the failed range (11),
        // which is above tip_height (10).
        assert_eq!(state.sync_height, Height::new(11));
    }

    // -- on_value_response: certificate height validation --

    /// Helper to create a RawDecidedValue with a given certificate height.
    fn make_raw_value(height: u64) -> crate::RawDecidedValue<TestContext> {
        use arc_malachitebft_test::ValueId;
        use bytes::Bytes;
        use malachitebft_core_types::{CommitCertificate, Round};

        crate::RawDecidedValue::new(
            Bytes::from_static(b"test"),
            CommitCertificate {
                height: Height::new(height),
                round: Round::ZERO,
                value_id: ValueId::new(height),
                commit_signatures: vec![],
            },
        )
    }

    /// Helper to set up test state with a pending request and a single peer.
    fn setup_response_test(
        range_start: u64,
        range_end: u64,
    ) -> (State<TestContext>, crate::Metrics, PeerId) {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        let peer = PeerId::random();
        state.peers.insert(
            peer,
            crate::Status {
                peer_id: peer,
                tip_height: Height::new(range_end + 10),
                history_min_height: Height::new(1),
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(range_start)..=Height::new(range_end),
                peer,
                excluded_peers: BTreeSet::new(),
            },
        );

        (state, metrics, peer)
    }

    fn has_process_value_response(effects: &[crate::Effect<TestContext>]) -> bool {
        effects
            .iter()
            .any(|e| matches!(e, crate::Effect::ProcessValueResponse(..)))
    }

    #[test]
    fn test_value_response_accepts_sequential_heights() {
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![
                make_raw_value(10),
                make_raw_value(11),
                make_raw_value(12),
                make_raw_value(13),
                make_raw_value(14),
            ],
        );

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            has_process_value_response(&effects),
            "Response with correct sequential heights should be accepted"
        );
    }

    #[test]
    fn test_value_response_rejects_duplicate_heights() {
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![
                make_raw_value(10),
                make_raw_value(10),
                make_raw_value(10),
                make_raw_value(10),
                make_raw_value(14),
            ],
        );

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            !has_process_value_response(&effects),
            "Response with duplicate heights should be rejected"
        );
    }

    #[test]
    fn test_value_response_rejects_out_of_order_heights() {
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![
                make_raw_value(10),
                make_raw_value(11),
                make_raw_value(13),
                make_raw_value(12),
                make_raw_value(14),
            ],
        );

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            !has_process_value_response(&effects),
            "Response with out-of-order heights should be rejected"
        );
    }

    #[test]
    fn test_value_response_rejects_heights_with_gaps() {
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        // Heights [10, 11, 12, 14, 15] — gap at 13, extra 15
        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![
                make_raw_value(10),
                make_raw_value(11),
                make_raw_value(12),
                make_raw_value(14),
                make_raw_value(15),
            ],
        );

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            !has_process_value_response(&effects),
            "Response with gaps in heights should be rejected"
        );
    }

    #[test]
    fn test_value_response_accepts_single_value() {
        let (mut state, metrics, peer) = setup_response_test(10, 10);

        let response = crate::ValueResponse::new(Height::new(10), vec![make_raw_value(10)]);

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            has_process_value_response(&effects),
            "Response with a single value at the correct height should be accepted"
        );
    }

    #[test]
    fn test_value_response_rejects_wrong_start_height() {
        // Requested range 10..=14, but peer sends sequential values starting at 5
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(
            Height::new(5),
            vec![
                make_raw_value(5),
                make_raw_value(6),
                make_raw_value(7),
                make_raw_value(8),
                make_raw_value(9),
            ],
        );

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            !has_process_value_response(&effects),
            "Response with sequential but wrong-range heights should be rejected"
        );
    }

    // -- on_got_decided_values: reject invalid host responses --

    /// Extract the `ValueResponse` from a `SendValueResponse` effect.
    fn extract_value_response(
        effects: &[crate::Effect<TestContext>],
    ) -> &crate::ValueResponse<TestContext> {
        effects
            .iter()
            .find_map(|e| match e {
                crate::Effect::SendValueResponse(_, response, _) => Some(response),
                _ => None,
            })
            .expect("expected a SendValueResponse effect")
    }

    #[test]
    fn test_on_got_decided_values_sends_valid_response() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        let values = vec![make_raw_value(5), make_raw_value(6), make_raw_value(7)];

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::GotDecidedValues(
                InboundRequestId::new("req1"),
                Height::new(5)..=Height::new(7),
                values,
            ),
        )
        .unwrap();

        let response = extract_value_response(&effects);
        assert_eq!(response.start_height, Height::new(5));
        assert_eq!(response.values.len(), 3);
    }

    #[test]
    fn test_on_got_decided_values_forwards_truncated_response() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        // Range expects 3 values (5..=7) but only 2 provided (e.g. truncated by engine).
        // A count mismatch alone should not prevent forwarding valid values.
        let values = vec![make_raw_value(5), make_raw_value(6)];

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::GotDecidedValues(
                InboundRequestId::new("req1"),
                Height::new(5)..=Height::new(7),
                values,
            ),
        )
        .unwrap();

        let response = extract_value_response(&effects);
        assert_eq!(response.start_height, Height::new(5));
        assert_eq!(response.values.len(), 2);
    }

    #[test]
    fn test_on_got_decided_values_truncates_at_wrong_height() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        // Range is 5..=7 but second value has height 10 instead of 6.
        // Only the valid prefix (height 5) should be forwarded.
        let values = vec![make_raw_value(5), make_raw_value(10), make_raw_value(7)];

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::GotDecidedValues(
                InboundRequestId::new("req1"),
                Height::new(5)..=Height::new(7),
                values,
            ),
        )
        .unwrap();

        let response = extract_value_response(&effects);
        assert_eq!(response.start_height, Height::new(5));
        assert_eq!(
            response.values.len(),
            1,
            "expected only the valid prefix, got {} values",
            response.values.len()
        );
    }

    #[test]
    fn test_on_got_decided_values_first_value_wrong_sends_empty() {
        let mut state = make_test_state();
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        // First value already has the wrong height — no valid prefix exists.
        let values = vec![make_raw_value(10), make_raw_value(6), make_raw_value(7)];

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::GotDecidedValues(
                InboundRequestId::new("req1"),
                Height::new(5)..=Height::new(7),
                values,
            ),
        )
        .unwrap();

        let response = extract_value_response(&effects);
        assert_eq!(response.start_height, Height::new(5));
        assert!(
            response.values.is_empty(),
            "expected empty response when first value is wrong, got {} values",
            response.values.len()
        );
    }
}

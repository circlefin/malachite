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
    perform, Effect, Error, HeightStartType, InboundRequestId, Metrics, OutboundFailureReason,
    OutboundRequestId, PeerId, PendingRequestEntry, RawDecidedValue, Request, Resume, State,
    Status, ValueRequest, ValueResponse,
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

    /// The network layer reported that an outbound sync request could not be
    /// delivered or completed (dial failure, connection closed mid-request,
    /// libp2p-level timeout, etc.).
    SyncRequestFailed(
        OutboundRequestId,
        PeerId,
        Request<Ctx>,
        OutboundFailureReason,
    ),

    /// A fault in a synced value (its certificate or its bytes) is attributable
    /// to the peer that served it: penalize and re-request from another peer.
    PeerFault(PeerId, Ctx::Height),

    /// Processing a synced value hit a local/transient failure (e.g. the
    /// execution layer being temporarily unavailable). No peer is to blame, so
    /// none is carried — re-request without penalizing or excluding anyone.
    LocalTransientError(Ctx::Height),

    /// A peer has disconnected
    PeerDisconnected(PeerId),
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

        Input::SyncRequestFailed(request_id, peer_id, request, reason) => {
            on_sync_request_failed(&co, state, metrics, request_id, peer_id, request, reason).await
        }

        Input::PeerFault(peer, value) => on_peer_fault(co, state, metrics, peer, value).await,

        Input::LocalTransientError(height) => {
            on_local_transient_error(co, state, metrics, height).await
        }

        Input::PeerDisconnected(peer_id) => {
            on_peer_disconnected(&co, state, metrics, peer_id).await
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

    let start = response.start_height;
    let received_len = response.values.len();
    let requested_len = requested_range.len();

    // A valid response starts at the requested start height and covers a
    // non-empty prefix (possibly all) of the requested range. Shorter prefixes
    // are accepted so peers returning truncated responses under
    // `max_response_size` still make progress; they are credited less through
    // `SyncResult::PartialSuccess` scoring below.
    let range_valid =
        start == *requested_range.start() && received_len > 0 && received_len <= requested_len;

    if !range_valid {
        warn!(
            %request_id, %peer_id,
            "Received response with wrong range: expected {} ({requested_len} values max), got {received_len} values starting at {start}",
            DisplayRange(requested_range),
        );

        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    if !validate_value_response_heights::<Ctx>(&response) {
        warn!(
            %request_id, %peer_id,
            "Received response with non-contiguous certificate heights for range {}",
            DisplayRange(requested_range),
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

    // Decisions are notified by independent tasks, so they can arrive out of
    // order. Keep the tip at the highest height seen: a lower tip shrinks the
    // read-ahead limit and can hold the request frontier below the height
    // consensus waits for.
    state.tip_height = max(state.tip_height, height);

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

    metrics.value_request_received(&request_id);

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
    let values_count = response.values.len();
    debug!(start = %start, num_values = %values_count, %peer_id, "Received response from peer");

    // Extract cheap Copy data from the pending entry. NLL releases the borrow
    // once the Copy values are bound, so mutable access to `state` is free
    // afterwards.
    let Some(entry) = state.pending_requests.get(&request_id) else {
        return Ok(());
    };
    let entry_peer = entry.peer;
    let range_start = *entry.range.start();
    let requested_len = entry.range.len();

    if entry_peer != peer_id {
        // Defensive check: `on_value_response` already rejects responses from
        // a different peer than the one recorded in the pending entry.
        error!(
            %request_id, peer.actual = %peer_id, peer.expected = %entry_peer,
            "Received response from different peer than expected"
        );
        return on_invalid_value_response(co, state, metrics, request_id, peer_id).await;
    }

    if let Some(response_time) = metrics.value_response_received(start.as_u64()) {
        let result = if values_count < requested_len {
            SyncResult::PartialSuccess {
                received: values_count,
                requested: requested_len,
                response_time,
            }
        } else {
            SyncResult::Success(response_time)
        };

        state
            .peer_scorer
            .update_score_with_metrics(peer_id, result, &metrics.scoring);
    }

    // Tell consensus to process the response.
    perform!(
        co,
        Effect::ProcessValueResponse(peer_id, request_id.clone(), response, Default::default())
    );

    if values_count < requested_len {
        // NOTE: We cannot simply call `re_request_values_from_peer_except` here.
        // Although we received some values from the peer, these values have not yet been processed
        // by the consensus engine. If we called `re_request_values_from_peer_except`, we would
        // end up re-requesting the entire original range (including values we already received),
        // causing the syncing peer to repeatedly send multiple requests until the already-received
        // values are fully processed.
        // To tackle this, we first update the current pending request with the range of values
        // we received, and then issue a new request for the remaining values.
        //
        // `on_value_response` guarantees `values_count >= 1` at this point, so the
        // `increment_by`/`decrement` arithmetic below is well-defined.
        let new_start = range_start.increment_by(values_count as u64);

        let entry = state.pending_requests.remove(&request_id).unwrap();
        let updated_range = range_start..=new_start.decrement().unwrap_or_default();
        state.update_request(
            request_id,
            peer_id,
            updated_range,
            entry.excluded_peers,
            false,
        );
        state.prune_pending_requests();

        // Return the suffix to the global frontier instead of scheduling it
        // directly, so the next request pass starts at the lowest uncovered
        // height rather than at the suffix of the range just answered.
        set_sync_height(state, min(state.sync_height, new_start));
        request_values(co, state, metrics).await?;
    } else {
        if let Some(entry) = state.pending_requests.get_mut(&request_id) {
            // Full response — the entry becomes a reservation. It keeps its range
            // reserved so the range is not requested twice, and it releases its
            // slot in the parallel-request budget. `prune_pending_requests` drops
            // it once consensus advances past the range.
            entry.inflight = false;
        }

        // Spend the released slot now. The other callers of `request_values` are
        // a peer status and the start of a height. Neither is guaranteed here:
        // consensus cannot start a height while a lower height is missing, and a
        // peer that has stopped deciding broadcasts no further status when status
        // updates are eager. Without this pass the released slot would stay idle
        // and the missing height would stay unrequested.
        request_values(co, state, metrics).await?;
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
    re_request_values_from_peer_except(&co, state, metrics, request_id, Some(peer_id)).await?;

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
            request_id.clone(),
            ValueResponse::new(*start, values),
            Default::default()
        )
    );

    metrics.value_response_sent(&request_id);

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

            // Ask the network layer to drop the now-abandoned request so any
            // late response is discarded at the source instead of triggering
            // downstream work (e.g. certificate fetches that hit upstream rate
            // limits, only to be dropped here as "unknown request ID").
            perform!(
                co,
                Effect::CancelValueRequest(request_id.clone(), Default::default())
            );

            re_request_values_from_peer_except(&co, state, metrics, request_id, Some(peer_id))
                .await?;
        }
    };

    Ok(())
}

pub async fn on_sync_request_failed<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer_id: PeerId,
    request: Request<Ctx>,
    reason: OutboundFailureReason,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match request {
        Request::ValueRequest(value_request) => {
            info!(%peer_id, ?reason, range = %DisplayRange(&value_request.range), "Sync request failed");

            state.peer_scorer.update_score(peer_id, SyncResult::Failure);

            metrics.value_request_failed(reason, value_request.range.start().as_u64());

            re_request_values_from_peer_except(co, state, metrics, request_id, Some(peer_id))
                .await?;
        }
    };

    Ok(())
}

async fn on_peer_disconnected<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.peers.remove(&peer_id).is_none() {
        // Peer never sent a status, so nothing to clean up.
        return Ok(());
    }

    info!(%peer_id, "Peer disconnected");

    // Re-request pending values assigned to this peer from another peer,
    // adding the disconnected peer to the exclusion set so it is not selected
    // again before reconnecting.
    //
    // Only requests still awaiting a response are re-issued. A reservation
    // already holds its values, which consensus has buffered, so re-requesting
    // it would take a request slot and buffer a second copy of every height in
    // its range. The reservation keeps its range reserved and its former owner
    // recorded until consensus advances past it and prunes it.
    let peer_request_ids: Vec<OutboundRequestId> = state
        .pending_requests
        .iter()
        .filter(|(_, entry)| entry.peer == peer_id && entry.inflight)
        .map(|(request_id, _)| request_id.clone())
        .collect();

    for request_id in peer_request_ids {
        re_request_values_from_peer_except(co, state, metrics, request_id, Some(peer_id)).await?;
    }

    Ok(())
}

async fn on_peer_fault<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%peer_id, %height, "Synced value fault is attributable to peer, penalizing");
    penalize_peer_and_retry(co, state, metrics, peer_id, height).await
}

/// Handle a local/transient failure while processing a synced value (e.g. the
/// execution layer being temporarily unavailable). No peer is to blame, so we
/// re-request the batch covering `height` without penalizing or excluding any
/// peer.
async fn on_local_transient_error<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    warn!(
        %height,
        "Transient local error while processing value, re-requesting without penalizing any peer"
    );

    if let Some((request_id, _stored_peer_id)) = state.get_request_id_by(height) {
        // `except_peer_id = None`: the failure is not attributable to a peer,
        // so re-request without adding anyone to the exclusion set.
        re_request_values_from_peer_except(&co, state, metrics, request_id, None).await?;
    } else {
        error!(%height, "Received height for unknown request");
    }

    Ok(())
}

// Penalize the peer and re-request the batch covering `height` from a different peer.
async fn penalize_peer_and_retry<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    state.peer_scorer.update_score(peer_id, SyncResult::Failure);

    if let Some((request_id, stored_peer_id)) = state.get_request_id_by(height) {
        if stored_peer_id != peer_id {
            // Defensive check: `on_value_response` already rejects responses from
            // a different peer than the one recorded in the pending entry.
            error!(
                %request_id, peer.actual = %peer_id, peer.expected = %stored_peer_id,
                "Received response from different peer than expected"
            );
        }
        re_request_values_from_peer_except(&co, state, metrics, request_id, Some(peer_id)).await?;
    } else {
        error!(%peer_id, %height, "Received height for unknown request");
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

    if state.inflight_requests() >= max_parallel_requests {
        info!(
            max_parallel_requests,
            inflight_requests = state.inflight_requests(),
            pending_requests = state.pending_requests.len(),
            "Maximum number of parallel requests reached, skipping request for values"
        );

        return Ok(());
    };

    while state.inflight_requests() < max_parallel_requests {
        // Find the next uncovered range starting from current sync_height
        let initial_height = state.sync_height;
        let range = find_next_uncovered_range_from::<Ctx>(
            initial_height,
            state.config.batch_size as u64,
            &state.pending_requests,
        );

        // Values are useless to consensus until every height below them is
        // decided, so stop once the frontier runs far enough ahead of the tip.
        let read_ahead_limit = state.read_ahead_limit();
        if *range.start() > read_ahead_limit {
            debug!(
                range = %DisplayRange(&range),
                tip_height = %state.tip_height,
                read_ahead_limit = %read_ahead_limit,
                "Read-ahead limit reached, skipping request for values"
            );
            break;
        }

        // Get a random peer that can provide the values in the range.
        let Some((peer, range)) = state.random_peer_with(&range) else {
            debug!("No peer to request sync from");
            // No connected peer reached this height yet, we can stop syncing here.
            break;
        };

        let tracked =
            send_and_track_request_to_peer(&co, state, metrics, peer, range, BTreeSet::new())
                .await?;

        if !tracked {
            // The send was not tracked, so the in-flight count is unchanged and
            // the same range stays uncovered. Stop the cycle and let a later
            // trigger (status update, decision, height start) resume it.
            debug!("Sync request was not tracked, stopping request cycle");
            break;
        }
    }

    Ok(())
}

/// Send a value request to `peer` and track it as a pending request on success.
///
/// Returns whether a request was tracked. `false` means the send was skipped or
/// failed at the transport level: no pending request is recorded and
/// `sync_height` is rolled back so the range is reconsidered on the next cycle.
async fn send_and_track_request_to_peer<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer: PeerId,
    range: RangeInclusive<<Ctx as Context>::Height>,
    excluded_peers: BTreeSet<PeerId>,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    // Capture the range start before `range` is moved into `send_request_to_peer`,
    // so we can roll sync_height back if the send is skipped.
    let range_start = *range.start();

    // Send the request
    let Some((request_id, final_range)) =
        send_request_to_peer(co, state, metrics, range, peer).await?
    else {
        // Request was skipped. `send_request_to_peer` returns `Ok(None)` in
        // three cases:
        //   1. The input range was empty (shouldn't happen from any known
        //      caller — all callers derive `range` from a pending entry or
        //      `find_next_uncovered_range_from`, both of which produce
        //      non-empty ranges).
        //   2. The range was trimmed empty because all heights in it have
        //      already been validated by consensus (tip advanced past the
        //      range mid-flight). The rollback is effectively a no-op here:
        //      `set_sync_height` raises the candidate back above
        //      `tip_height + 1`.
        //   3. The `SendValueRequest` effect yielded `None` (transport-level
        //      failure). This is the failure mode the rollback actually
        //      targets.
        //
        // Roll sync_height back towards the range start so the range can be
        // picked up again by `find_next_uncovered_range_from` on the next
        // request cycle. Without this, a retry path that popped the pending
        // entry before calling us would leave sync_height past an untracked
        // range and stall sync for that segment until an external event.
        set_sync_height(state, min(state.sync_height, range_start));
        return Ok(false);
    };

    // Store the pending request
    state.pending_requests.insert(
        request_id,
        PendingRequestEntry {
            range: final_range.clone(),
            peer,
            excluded_peers,
            inflight: true,
        },
    );

    // Advance sync_height past this range only if it sat inside the range.
    // If sync_height is already below the range — because a concurrent
    // exhaustion path rewound it to an earlier untracked range — leave it
    // alone so the next request cycle picks that range up.
    if final_range.contains(&state.sync_height) {
        set_sync_height(state, final_range.end().increment());
    }

    Ok(true)
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
    co: &Co<Ctx>,
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

    // A reservation holds no request slot, so re-requesting one adds a request
    // the budget never accounted for. An entry that was still in flight gave its
    // own slot back when it was removed above, so a retry is never blocked here.
    if state.inflight_requests() >= state.max_parallel_requests() {
        debug!(
            %request_id,
            inflight_requests = state.inflight_requests(),
            max_parallel_requests = state.max_parallel_requests(),
            "Parallel request budget is full, leaving the range to the next request pass"
        );

        set_sync_height(state, min(state.sync_height, *entry.range.start()));
        return Ok(());
    }

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

    let to_request_end = *entry.range.end();
    let peer_offered_end = *peer_range.end();

    // NOTE: `entry.excluded_peers` is moved into `send_and_track_request_to_peer`
    // and is dropped if the send fails (`Ok(None)` branch). This is
    // intentional — the next request cycle starts with a clean exclusion
    // set, and convergence away from unhealthy peers is left to the peer
    // scorer, which biases `random_peer_with_except`'s selection (timeouts
    // and invalid responses from the same peer will score it down).
    let _ =
        send_and_track_request_to_peer(co, state, metrics, peer, peer_range, entry.excluded_peers)
            .await?;

    // Keep any suffix omitted by a prefix-only retry on the next request frontier.
    if peer_offered_end < to_request_end {
        set_sync_height(state, min(state.sync_height, peer_offered_end.increment()));
    }

    Ok(())
}

/// Set `sync_height` to the given candidate while enforcing both invariants:
///   - `sync_height > tip_height`
///   - `sync_height` is not covered by any pending request
///
/// If the candidate violates either invariant, it is raised to the next
/// uncovered height at or above `tip_height + 1`.
///
/// Only the pending-range skip is logged: callers are expected to pass a
/// candidate outside every pending range, so a skip there means an unsafe
/// value was passed in. The tip-floor bump (candidate ≤ tip_height) is the
/// routine post-decide/rollback path and is not logged.
fn set_sync_height<Ctx: Context>(state: &mut State<Ctx>, candidate: Ctx::Height) {
    let floor = max(state.tip_height.increment(), candidate);
    let new_sync_height = find_next_uncovered_height::<Ctx>(floor, &state.pending_requests);

    if new_sync_height != floor {
        warn!(
            %candidate,
            tip_height = %state.tip_height,
            floor = %floor,
            sync_height = %new_sync_height,
            "sync_height candidate is inside a pending request range; advancing past it"
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
    use malachitebft_core_types::Round;
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
                        inflight: true,
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
                        inflight: true,
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
                        inflight: true,
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

    // Helper: drive a handle::Input through the coroutine-based handler.
    // Collects all yielded effects and invokes `resume_strategy` to produce
    // the `Resume` value fed back for each yielded effect. Tests that do not
    // need meaningful resume values should prefer the `drive_input` wrapper,
    // which always resumes with the default.
    fn drive_input_with<F>(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
        mut resume_strategy: F,
    ) -> Result<Vec<crate::Effect<TestContext>>, crate::Error<TestContext>>
    where
        F: FnMut(&crate::Effect<TestContext>) -> crate::Resume<TestContext>,
    {
        use crate::co::{CoState, Gen};
        use crate::Resume;

        let mut effects = Vec::new();
        let mut gen = Gen::new(|co| handle(co, state, metrics, input));
        let mut result = gen.resume_with(Resume::default());

        loop {
            match result {
                CoState::Yielded(effect) => {
                    let resume = resume_strategy(&effect);
                    effects.push(effect);
                    result = gen.resume_with(resume);
                }
                CoState::Complete(r) => return r.map(|()| effects),
            }
        }
    }

    // Helper: drive a handle::Input and auto-resume every effect with the
    // default resume value. Only safe for inputs whose handling does not
    // require meaningful resume values (no-peer / no-yield paths).
    fn drive_input(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
    ) -> Result<Vec<crate::Effect<TestContext>>, crate::Error<TestContext>> {
        drive_input_with(state, metrics, input, |_| crate::Resume::default())
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
                inflight: true,
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
                inflight: true,
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req2"),
            PendingRequestEntry {
                range: Height::new(111)..=Height::new(120),
                peer: peer_b,
                excluded_peers: BTreeSet::new(),
                inflight: true,
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
                inflight: true,
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
                inflight: true,
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
        use crate::Resume;
        let mut req_counter = 0u64;
        drive_input_with(state, metrics, input, |effect| match effect {
            Effect::SendValueRequest(..) => {
                req_counter += 1;
                Resume::ValueRequestId(Some(OutboundRequestId::new(format!(
                    "retry_req{req_counter}"
                ))))
            }
            _ => Resume::default(),
        })
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
                inflight: true,
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

    // peer_a can serve the whole range 11..=15; peer_b only the prefix 11..=13
    // (tip 13). Pending request req1 (11..=15) is assigned to peer_a; sync_height
    // sits at 16 (already advanced past the range when req1 was sent).
    fn setup_prefix_retry_state() -> (State<TestContext>, crate::Metrics, PeerId, PeerId) {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(15),
                history_min_height: Height::new(1),
            },
        );
        state.peers.insert(
            peer_b,
            crate::Status {
                peer_id: peer_b,
                tip_height: Height::new(13),
                history_min_height: Height::new(1),
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        (state, metrics, peer_a, peer_b)
    }

    fn assert_retry_preserves_coverage(state: &State<TestContext>, peer_a: PeerId, peer_b: PeerId) {
        assert!(
            state.pending_requests.values().any(|e| e.peer == peer_b
                && e.range == (Height::new(11)..=Height::new(13))
                && e.excluded_peers.contains(&peer_a)),
            "Expected a prefix re-request 11..=13 to peer_b excluding peer_a; \
             pending_requests = {:?}",
            state.pending_requests,
        );

        let uncovered: Vec<u64> = ((state.tip_height.as_u64() + 1)..state.sync_height.as_u64())
            .filter(|h| {
                !state
                    .pending_requests
                    .values()
                    .any(|e| e.range.start().as_u64() <= *h && *h <= e.range.end().as_u64())
            })
            .collect();
        assert!(
            uncovered.is_empty(),
            "Uncovered heights {uncovered:?}: above tip_height ({}) and below sync_height ({})",
            state.tip_height.as_u64(),
            state.sync_height.as_u64(),
        );
    }

    #[test]
    fn test_timeout_retry_to_prefix_peer_preserves_coverage() {
        let (mut state, metrics, peer_a, peer_b) = setup_prefix_retry_state();

        drive_input_with_retries(
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

        assert_retry_preserves_coverage(&state, peer_a, peer_b);
    }

    #[test]
    fn test_disconnect_retry_to_prefix_peer_preserves_coverage() {
        let (mut state, metrics, peer_a, peer_b) = setup_prefix_retry_state();

        drive_input_with_retries(&mut state, &metrics, Input::PeerDisconnected(peer_a)).unwrap();

        assert!(
            !state.peers.contains_key(&peer_a),
            "Disconnected peer_a should be removed from the peers map"
        );
        assert_retry_preserves_coverage(&state, peer_a, peer_b);
    }

    #[test]
    fn test_re_request_preserves_rewound_sync_height_below_other_pending_range() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(21);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(30),
                history_min_height: Height::new(1),
            },
        );
        state.peers.insert(
            peer_b,
            crate::Status {
                peer_id: peer_b,
                tip_height: Height::new(30),
                history_min_height: Height::new(1),
            },
        );

        // Entry A holds heights 11..=15 and has already excluded peer_a, so
        // when peer_b also times out, every eligible peer is exhausted and
        // sync_height rewinds to 11.
        state.pending_requests.insert(
            OutboundRequestId::new("req_a"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_b,
                excluded_peers: [peer_a].into_iter().collect(),
                inflight: true,
            },
        );

        // Entry B holds heights 16..=20 with peer_a still available as a
        // retry target.
        state.pending_requests.insert(
            OutboundRequestId::new("req_b"),
            PendingRequestEntry {
                range: Height::new(16)..=Height::new(20),
                peer: peer_b,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        // Entry A times out on peer_b — all eligible peers exhausted.
        drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req_a"),
                peer_b,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
            ),
        )
        .unwrap();

        assert_eq!(
            state.sync_height,
            Height::new(11),
            "exhaustion should rewind sync_height to the start of the failed range"
        );

        // Entry B times out on peer_b — peer_a is still eligible, so a new
        // request goes out. sync_height must stay at 11 so the next request
        // cycle picks up the 11..=15 range that the exhaustion rewound to.
        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req_b"),
                peer_b,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(16)..=Height::new(20),
                )),
            ),
        )
        .unwrap();

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "Expected a re-request to be sent for entry B"
        );

        let req_b_entry = state
            .pending_requests
            .values()
            .find(|entry| entry.range == (Height::new(16)..=Height::new(20)))
            .expect("re-requested entry for 16..=20 should be present");
        assert_eq!(req_b_entry.peer, peer_a);

        assert_eq!(state.sync_height, Height::new(11));
    }

    /// The value-sync wedge invariant: every height strictly between
    /// `tip_height` and `sync_height` must be covered by some pending request.
    ///
    /// If a height in that window is covered by nothing, the forward-only
    /// request scanner (`find_next_uncovered_range_from`, which only ever looks
    /// forward from `sync_height`) can never revisit it, so consensus starves on
    /// it indefinitely.
    fn assert_no_uncovered_gap_below_sync_height(state: &State<TestContext>) {
        let mut height = state.tip_height.increment();
        while height < state.sync_height {
            let covered = state
                .pending_requests
                .values()
                .any(|entry| entry.range.contains(&height));
            assert!(
                covered,
                "height {} lies below sync_height {} but is covered by no pending request \
                 — this is the value-sync wedge gap",
                height.as_u64(),
                state.sync_height.as_u64(),
            );
            height = height.increment();
        }
    }

    /// Exhaustion of a low range rewinds `sync_height` so the forward request
    /// scanner revisits it. A concurrent successful re-request of a higher range
    /// must not clobber that rewind and strand needed heights below `sync_height`.
    ///
    /// This exercises five parallel slots, exhaustion on the lowest range, and a
    /// concurrent successful re-request of a higher range. No height below
    /// `sync_height` may remain uncovered, and the next request cycle must request
    /// the height consensus needs.
    #[test]
    fn test_value_sync_wedge_does_not_form_after_exhaustion_storm() {
        let mut state = make_test_state();
        state.started = true;
        state.config.parallel_requests = 5;
        state.config.batch_size = 5;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        // Consensus is stuck needing height 11; sync_height sits past five
        // in-flight batches covering 11..=35.
        state.tip_height = Height::new(10);
        state.sync_height = Height::new(36);

        let peer_a = PeerId::random(); // healthy: can serve every range
        let peer_b = PeerId::random(); // the flaky peer the requests are routed to

        for &peer in &[peer_a, peer_b] {
            state.peers.insert(
                peer,
                crate::Status {
                    peer_id: peer,
                    tip_height: Height::new(100),
                    history_min_height: Height::new(1),
                },
            );
        }

        // Lowest batch (11..=15) — the one consensus needs first. It already has
        // peer_a excluded and is routed to peer_b, so when peer_b times out every
        // eligible peer is exhausted and sync_height rewinds to 11.
        state.pending_requests.insert(
            OutboundRequestId::new("req_11_15"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_b,
                excluded_peers: [peer_a].into_iter().collect(),
                inflight: true,
            },
        );
        // Four higher batches routed to peer_b with peer_a still available.
        for (id, start, end) in [
            ("req_16_20", 16, 20),
            ("req_21_25", 21, 25),
            ("req_26_30", 26, 30),
            ("req_31_35", 31, 35),
        ] {
            state.pending_requests.insert(
                OutboundRequestId::new(id),
                PendingRequestEntry {
                    range: Height::new(start)..=Height::new(end),
                    peer: peer_b,
                    excluded_peers: BTreeSet::new(),
                    inflight: true,
                },
            );
        }

        // 1. The lowest batch times out on peer_b → all eligible peers exhausted
        //    → sync_height rewinds to the start of the needed range (11).
        drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req_11_15"),
                peer_b,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
            ),
        )
        .unwrap();

        assert_eq!(
            state.sync_height,
            Height::new(11),
            "exhaustion should rewind sync_height to the start of the needed range"
        );

        // 2. A higher batch times out on peer_b → re-request succeeds on peer_a.
        //    An unconditional forward set would move sync_height to 21 and strand
        //    11..=15 below it. The conditional advance must leave it at 11.
        drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req_16_20"),
                peer_b,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(16)..=Height::new(20),
                )),
            ),
        )
        .unwrap();

        // The needed range 11..=15 is no longer pending (exhausted), but it must
        // not be stranded below sync_height.
        assert_no_uncovered_gap_below_sync_height(&state);
        assert_eq!(
            state.sync_height,
            Height::new(11),
            "successful re-request of a higher range must not clobber the rewind"
        );

        // 3. The next request cycle (driven by a peer status) must re-request the
        //    needed range starting at 11 — proving the node recovers rather than
        //    wedging on the skipped-over height.
        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::Status(crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(100),
                history_min_height: Height::new(1),
            }),
        )
        .unwrap();

        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::SendValueRequest(_, req, _) if req.range.contains(&Height::new(11))
            )),
            "next request cycle must re-request the needed height 11"
        );
    }

    // -- on_sync_request_failed: libp2p outbound failure handling --

    #[test]
    fn test_sync_request_failed_penalizes_peer_and_reroutes() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

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

        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        let initial_score = state.peer_scorer.get_score(&peer_a);

        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::SyncRequestFailed(
                OutboundRequestId::new("req1"),
                peer_a,
                crate::Request::ValueRequest(crate::ValueRequest::new(
                    Height::new(11)..=Height::new(15),
                )),
                OutboundFailureReason::ConnectionClosed,
            ),
        )
        .unwrap();

        assert!(
            state.peer_scorer.get_score(&peer_a) < initial_score,
            "Peer A should be penalized when its sync request fails"
        );

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "Expected a re-request after SyncRequestFailed"
        );

        assert_eq!(state.pending_requests.len(), 1);
        let (_, entry) = state.pending_requests.iter().next().unwrap();
        assert_eq!(entry.peer, peer_b, "Retry should go to peer B");
        assert!(
            entry.excluded_peers.contains(&peer_a),
            "Peer A should be in the excluded set"
        );
    }

    // -- on_local_transient_error: local/transient fault handling --

    #[test]
    fn test_local_transient_error_re_requests_without_penalizing_or_excluding_peer() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        // A single peer on purpose: a local/transient failure (e.g. the
        // execution layer being down) must not exclude the only peer we have.
        let peer_a = PeerId::random();

        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );

        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        let initial_score = state.peer_scorer.get_score(&peer_a);

        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::LocalTransientError(Height::new(11)),
        )
        .unwrap();

        assert_eq!(
            state.peer_scorer.get_score(&peer_a),
            initial_score,
            "Peer A must not be penalized for a local/transient processing error"
        );

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "Expected a re-request after LocalTransientError"
        );

        assert_eq!(state.pending_requests.len(), 1);
        let (_, entry) = state.pending_requests.iter().next().unwrap();
        assert_eq!(
            entry.peer, peer_a,
            "Retry may go back to peer A since it is not at fault"
        );
        assert!(
            entry.excluded_peers.is_empty(),
            "No peer should be excluded for a local/transient processing error"
        );
    }

    // -- on_peer_disconnected: requests to disconnected peer get rerouted --

    #[test]
    fn test_peer_disconnected_reroutes_pending_requests_to_alternate_peer() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(21);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();
        let peer_c = PeerId::random();

        for &peer in &[peer_a, peer_b, peer_c] {
            state.peers.insert(
                peer,
                crate::Status {
                    peer_id: peer,
                    tip_height: Height::new(30),
                    history_min_height: Height::new(1),
                },
            );
        }

        // Two pending requests assigned to peer_a, one to peer_b.
        state.pending_requests.insert(
            OutboundRequestId::new("req_a1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req_a2"),
            PendingRequestEntry {
                range: Height::new(16)..=Height::new(20),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req_b1"),
            PendingRequestEntry {
                range: Height::new(21)..=Height::new(25),
                peer: peer_b,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        let effects =
            drive_input_with_retries(&mut state, &metrics, Input::PeerDisconnected(peer_a))
                .unwrap();

        assert!(
            !state.peers.contains_key(&peer_a),
            "Disconnected peer should be removed from peers map",
        );

        let send_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::SendValueRequest(..)))
            .count();
        assert_eq!(
            send_count, 2,
            "Both requests to the disconnected peer should be re-issued"
        );

        assert_eq!(state.pending_requests.len(), 3);

        for entry in state.pending_requests.values() {
            assert_ne!(
                entry.peer, peer_a,
                "Retry must not be routed back to the disconnected peer"
            );
        }

        let rerouted_ranges: BTreeSet<(u64, u64)> = state
            .pending_requests
            .values()
            .filter(|entry| entry.excluded_peers.contains(&peer_a))
            .map(|entry| (entry.range.start().as_u64(), entry.range.end().as_u64()))
            .collect();
        assert!(
            rerouted_ranges.contains(&(11, 15)) && rerouted_ranges.contains(&(16, 20)),
            "Both ranges originally assigned to peer A should carry it in their exclusion set"
        );

        // Peer B's request must remain untouched.
        let peer_b_entry = state
            .pending_requests
            .get(&OutboundRequestId::new("req_b1"))
            .expect("Peer B's request should be preserved");
        assert_eq!(peer_b_entry.peer, peer_b);
        assert!(peer_b_entry.excluded_peers.is_empty());
    }

    #[test]
    fn test_peer_disconnected_leaves_reservations_in_place() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(21);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        for &peer in &[peer_a, peer_b] {
            state.peers.insert(
                peer,
                crate::Status {
                    peer_id: peer,
                    tip_height: Height::new(30),
                    history_min_height: Height::new(1),
                },
            );
        }

        // Peer A owns one outstanding request and one reservation whose values arrived.
        state.pending_requests.insert(
            OutboundRequestId::new("req_inflight"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req_reservation"),
            PendingRequestEntry {
                range: Height::new(16)..=Height::new(20),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: false,
            },
        );

        let effects =
            drive_input_with_retries(&mut state, &metrics, Input::PeerDisconnected(peer_a))
                .unwrap();

        // Only the outstanding request is re-issued.
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(11)..=Height::new(15)]
        );

        // The reservation keeps its range, its owner and its slot-free status.
        let reservation = state
            .pending_requests
            .get(&OutboundRequestId::new("req_reservation"))
            .expect("Reservation should be preserved");
        assert_eq!(reservation.range, Height::new(16)..=Height::new(20));
        assert_eq!(reservation.peer, peer_a);
        assert!(!reservation.inflight);
        assert_eq!(state.inflight_requests(), 1);
    }

    #[test]
    fn test_peer_disconnected_clears_pending_when_no_alternate_peer() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );

        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        let effects =
            drive_input_with_retries(&mut state, &metrics, Input::PeerDisconnected(peer_a))
                .unwrap();

        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "No request should be issued when no alternate peer is available"
        );
        assert!(state.pending_requests.is_empty());
        // sync_height rolls back to the start of the failed range (11), preserving the
        // invariant sync_height > tip_height (10).
        assert_eq!(state.sync_height, Height::new(11));
    }

    #[test]
    fn test_peer_disconnected_for_untracked_peer_is_noop() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let unknown_peer = PeerId::random();

        let effects =
            drive_input(&mut state, &metrics, Input::PeerDisconnected(unknown_peer)).unwrap();

        assert!(effects.is_empty());
        assert!(state.pending_requests.is_empty());
        assert!(state.peers.is_empty());
    }

    // -- on_value_response: certificate height validation --

    /// Helper to create a RawDecidedValue with a given certificate height.
    fn make_raw_value(height: u64) -> crate::RawDecidedValue<TestContext> {
        use arc_malachitebft_test::ValueId;
        use bytes::Bytes;
        use malachitebft_core_types::{ExtendedCommitCertificate, Round};

        crate::RawDecidedValue::new(
            Bytes::from_static(b"test"),
            ExtendedCommitCertificate {
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
                inflight: true,
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

    #[test]
    fn test_value_response_accepts_prefix_of_requested_range() {
        // Requested 5 values (10..=14) but peer returns a 3-value prefix.
        // Accepting the prefix triggers a follow-up `SendValueRequest` for the
        // remaining suffix, so drive through the retry-aware helper.
        let (mut state, metrics, peer) = setup_response_test(10, 14);
        // A second peer is needed so the suffix re-request has somewhere to go.
        let other_peer = PeerId::random();
        state.peers.insert(
            other_peer,
            crate::Status {
                peer_id: other_peer,
                tip_height: Height::new(24),
                history_min_height: Height::new(1),
            },
        );

        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![make_raw_value(10), make_raw_value(11), make_raw_value(12)],
        );

        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            has_process_value_response(&effects),
            "A non-empty prefix of the requested range must be accepted so the \
             requester can make progress when peers truncate under max_response_size"
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, crate::Effect::SendValueRequest(..))),
            "A prefix response must trigger a re-request for the remaining suffix"
        );
    }

    #[test]
    fn test_partial_response_after_status_change_preserves_coverage() {
        let mut state = make_test_state();
        state.started = true;
        state.tip_height = Height::new(10);
        state.sync_height = Height::new(21);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        let peer_a = PeerId::random();
        state.peers.insert(
            peer_a,
            crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(20),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        // The original peer has pruned past the remaining range's start since
        // accepting the request, while another peer can serve only a prefix.
        drive_input(
            &mut state,
            &metrics,
            Input::Status(crate::Status {
                peer_id: peer_a,
                tip_height: Height::new(20),
                history_min_height: Height::new(16),
            }),
        )
        .unwrap();

        let peer_b = PeerId::random();
        drive_input(
            &mut state,
            &metrics,
            Input::Status(crate::Status {
                peer_id: peer_b,
                tip_height: Height::new(17),
                history_min_height: Height::new(1),
            }),
        )
        .unwrap();

        let response = crate::ValueResponse::new(
            Height::new(11),
            vec![
                make_raw_value(11),
                make_raw_value(12),
                make_raw_value(13),
                make_raw_value(14),
            ],
        );

        drive_input_with_retries(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer_a, Some(response)),
        )
        .unwrap();

        assert!(state.pending_requests.values().any(|entry| {
            entry.peer == peer_b && entry.range == (Height::new(15)..=Height::new(17))
        }));
        assert_eq!(state.sync_height, Height::new(21));

        let uncovered: Vec<u64> = ((state.tip_height.as_u64() + 1)..state.sync_height.as_u64())
            .filter(|height| {
                !state
                    .pending_requests
                    .values()
                    .any(|entry| entry.range.contains(&Height::new(*height)))
            })
            .collect();
        assert!(
            uncovered.is_empty(),
            "Uncovered heights {uncovered:?}: above tip_height ({}) and below sync_height ({})",
            state.tip_height.as_u64(),
            state.sync_height.as_u64(),
        );
    }

    #[test]
    fn test_value_response_rejects_empty_response() {
        // Requested 10..=14 but peer returns zero values.
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(Height::new(10), vec![]);

        let effects = drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        assert!(
            !has_process_value_response(&effects),
            "An empty response is indistinguishable from a denial and must be rejected"
        );
    }

    #[test]
    fn test_value_response_rejects_longer_than_requested() {
        // Requested 5 values (10..=14) but peer returns 6 sequential values.
        let (mut state, metrics, peer) = setup_response_test(10, 14);

        let response = crate::ValueResponse::new(
            Height::new(10),
            vec![
                make_raw_value(10),
                make_raw_value(11),
                make_raw_value(12),
                make_raw_value(13),
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
            "A response longer than the requested range must be rejected"
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
    fn test_same_height_value_requests_record_independent_server_latencies() {
        let mut state = make_test_state();
        state.tip_height = Height::new(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let peer = PeerId::random();
        let range = Height::new(5)..=Height::new(5);

        for request_id in ["req1", "req2"] {
            drive_input(
                &mut state,
                &metrics,
                Input::ValueRequest(
                    InboundRequestId::new(request_id),
                    peer,
                    ValueRequest::new(range.clone()),
                ),
            )
            .unwrap();
        }

        for request_id in ["req1", "req2"] {
            drive_input(
                &mut state,
                &metrics,
                Input::GotDecidedValues(
                    InboundRequestId::new(request_id),
                    range.clone(),
                    vec![make_raw_value(5)],
                ),
            )
            .unwrap();
        }

        assert_eq!(metrics.value_server_latency_observation_count(), 2);
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

    // -- sync_height rollback on retry send failure / missing peer --

    /// Like [`drive_input_with_retries`] but resumes every `SendValueRequest`
    /// effect with `None`, simulating the case where the underlying send
    /// effect fails to produce a request id.
    fn drive_input_with_send_failures(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
    ) -> Result<Vec<crate::Effect<TestContext>>, crate::Error<TestContext>> {
        use crate::Resume;
        drive_input_with(state, metrics, input, |effect| match effect {
            Effect::SendValueRequest(..) => Resume::ValueRequestId(None),
            _ => Resume::default(),
        })
    }

    #[test]
    fn test_re_request_rolls_back_sync_height_on_send_failure() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

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

        // Pending request assigned to peer A for heights 11..=15.
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        // Peer A times out. A replacement peer (B) is available, but the send
        // effect fails — simulating a network / transport error.
        let effects = drive_input_with_send_failures(
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

        // A SendValueRequest was attempted (the retry path was taken).
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SendValueRequest(..))),
            "Expected a retry SendValueRequest effect"
        );

        // The original pending entry was consumed and no new one was inserted
        // because the send failed.
        assert!(
            state.pending_requests.is_empty(),
            "Pending requests should be empty when retry send fails"
        );

        // sync_height should be rolled back to the range start (11), which is
        // still above tip_height (10). Without the rollback, sync_height would
        // have remained at 16 and the range 11..=15 would never be retried.
        assert_eq!(state.sync_height, Height::new(11));
    }

    /// A transport-level send failure inside the request loop produces a single
    /// attempt and stops the cycle: the in-flight count does not advance, so the
    /// loop does not re-select the identical range. The range stays the sync
    /// target (sync_height preserved above tip) for a later trigger to resume.
    #[test]
    fn test_request_values_attempts_once_and_stops_on_send_failure() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(11);

        // A status from a peer ahead of us triggers the request loop. Its
        // `update_status` adds the peer to the routable set, so a peer is always
        // available to select.
        let peer = PeerId::random();
        let status = crate::Status {
            peer_id: peer,
            tip_height: Height::new(20),
            history_min_height: Height::new(1),
        };

        let effects =
            drive_input_with_send_failures(&mut state, &metrics, Input::Status(status)).unwrap();

        let send_attempts = effects
            .iter()
            .filter(|e| matches!(e, Effect::SendValueRequest(..)))
            .count();

        assert_eq!(
            send_attempts, 1,
            "request loop should attempt the send once per cycle"
        );
        assert!(
            state.pending_requests.is_empty(),
            "no request is tracked when the send fails"
        );
        // The range remains the sync target so a later trigger can resume it.
        assert_eq!(state.sync_height, Height::new(11));
    }

    #[test]
    fn test_timeout_emits_cancel_value_request_before_retry() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer_a = PeerId::random();
        let peer_b = PeerId::random();
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

        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer: peer_a,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

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

        // The cancellation effect must be emitted, and it must reference the
        // request ID that just timed out — so the network layer drops the
        // right in-flight request.
        let cancel_pos = effects.iter().position(|e| {
            matches!(
                e,
                Effect::CancelValueRequest(id, _) if id == &OutboundRequestId::new("req1")
            )
        });
        let send_pos = effects
            .iter()
            .position(|e| matches!(e, Effect::SendValueRequest(..)));

        let cancel_pos = cancel_pos.expect("CancelValueRequest must be emitted on timeout");
        let send_pos = send_pos.expect("Retry SendValueRequest must be emitted on timeout");

        // Cancel before retry: the late response should be dropped before we
        // commit to a new in-flight request.
        assert!(
            cancel_pos < send_pos,
            "CancelValueRequest (idx {cancel_pos}) must precede the retry SendValueRequest (idx {send_pos})"
        );
    }

    #[test]
    fn test_request_values_range_rolls_back_sync_height_when_no_peer_available() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer = PeerId::random();

        // Peer can only serve up to height 12 — it cannot cover the suffix 13..=15.
        state.peers.insert(
            peer,
            crate::Status {
                peer_id: peer,
                tip_height: Height::new(12),
                history_min_height: Height::new(1),
            },
        );

        // Pending request assigned to the peer for heights 11..=15.
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        // Peer returns a partial response covering only heights 11..=12.
        let response = crate::ValueResponse::new(
            Height::new(11),
            vec![make_raw_value(11), make_raw_value(12)],
        );

        drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        // The suffix 13..=15 cannot be served by any peer. sync_height must
        // roll back to the suffix start (13) so it is retried once a peer
        // advances past it.
        assert_eq!(state.sync_height, Height::new(13));

        // No pending request exists for the suffix.
        for entry in state.pending_requests.values() {
            assert!(
                !entry.range.contains(&Height::new(13)),
                "No pending request should cover the un-retried suffix start",
            );
        }
    }

    // Drive an input, assigning a fresh sequential request id to every
    // `SendValueRequest` effect, starting at `next_id`.
    fn drive_input_numbering_requests(
        state: &mut State<TestContext>,
        metrics: &crate::Metrics,
        input: Input<TestContext>,
        next_id: &mut u64,
    ) -> Vec<crate::Effect<TestContext>> {
        use crate::Resume;

        drive_input_with(state, metrics, input, |effect| match effect {
            Effect::SendValueRequest(..) => {
                *next_id += 1;
                Resume::ValueRequestId(Some(OutboundRequestId::new(format!("req{next_id}"))))
            }
            _ => Resume::default(),
        })
        .unwrap()
    }

    fn status(peer: PeerId, tip: u64) -> crate::Status<TestContext> {
        crate::Status {
            peer_id: peer,
            tip_height: Height::new(tip),
            history_min_height: Height::new(1),
        }
    }

    fn requested_ranges(effects: &[crate::Effect<TestContext>]) -> Vec<RangeInclusive<Height>> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::SendValueRequest(_, request, _) => Some(request.range.clone()),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn test_received_entries_do_not_block_re_request_of_lower_missing_height() {
        let mut state = make_test_state();
        state.config = crate::Config::default()
            .with_parallel_requests(2)
            .with_batch_size(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let mut next_id = 0;

        let peer = PeerId::random();

        // Node is at tip 0 and needs heights 1, 2 and 3.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(1), HeightStartType::Start),
            &mut next_id,
        );
        assert_eq!(state.tip_height, Height::new(0));
        assert_eq!(state.sync_height, Height::new(1));

        // The peer holds only height 1, so the request is trimmed to 1..=1.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 1)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(1)..=Height::new(1)]
        );

        // The peer advances to height 3, so heights 2..=3 are requested too.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 3)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(2)..=Height::new(3)]
        );
        assert_eq!(state.pending_requests.len(), 2);

        // The request for height 1 times out. The peer is excluded for the
        // retry and no other peer can serve height 1, so the entry is dropped.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::SyncRequestTimedOut(
                OutboundRequestId::new("req1"),
                peer,
                Request::ValueRequest(ValueRequest::new(Height::new(1)..=Height::new(1))),
            ),
            &mut next_id,
        );
        assert_eq!(state.sync_height, Height::new(1));
        assert_eq!(state.pending_requests.len(), 1);

        // The response to 2..=3 covers only height 2. The prefix is kept as a
        // reservation and the suffix returns to the frontier. Consensus still
        // cannot advance, and this response is the last input the node
        // receives: no further status arrives, and no height can start while
        // height 1 is missing. The released slot must go to height 1.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::ValueResponse(
                OutboundRequestId::new("req2"),
                peer,
                Some(crate::ValueResponse::new(
                    Height::new(2),
                    vec![make_raw_value(2)],
                )),
            ),
            &mut next_id,
        );

        assert_eq!(state.tip_height, Height::new(0));

        // The answered prefix is kept as a reservation, so it holds no slot.
        // Without this the pass below would still find a free slot and the
        // assertion on height 1 would pass for the wrong reason.
        let prefix = state
            .pending_requests
            .values()
            .find(|entry| entry.range == (Height::new(2)..=Height::new(2)))
            .expect("The answered prefix should stay reserved");
        assert!(!prefix.inflight);

        // The pass takes both the gap below the prefix and the suffix above it.
        assert_eq!(state.inflight_requests(), 2);

        assert!(
            state
                .pending_requests
                .values()
                .any(|entry| entry.inflight && entry.range.contains(&Height::new(1))),
            "height 1 was never re-requested: sync_height={}, pending={:?}",
            state.sync_height,
            state
                .pending_requests
                .values()
                .map(|entry| format!("{}", DisplayRange(&entry.range)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_full_response_releases_parallel_request_slot() {
        let mut state = make_test_state();
        state.config = crate::Config::default()
            .with_parallel_requests(2)
            .with_batch_size(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let mut next_id = 0;

        let peer = PeerId::random();

        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(1), HeightStartType::Start),
            &mut next_id,
        );

        // The peer holds heights 1 to 3 only, so the first request is trimmed.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 3)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(1)..=Height::new(3)]
        );

        // The peer catches up, so the next batch takes the second slot.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 100)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(4)..=Height::new(8)]
        );
        assert_eq!(state.inflight_requests(), 2);

        // A full response leaves a reservation behind, releases its slot, and
        // spends that slot on the next uncovered range straight away.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::ValueResponse(
                OutboundRequestId::new("req1"),
                peer,
                Some(crate::ValueResponse::new(
                    Height::new(1),
                    (1..=3).map(make_raw_value).collect(),
                )),
            ),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(9)..=Height::new(13)]
        );

        // The answered range stays reserved but holds no request.
        let reservation = state
            .pending_requests
            .get(&OutboundRequestId::new("req1"))
            .expect("Answered range should stay reserved");
        assert_eq!(reservation.range, Height::new(1)..=Height::new(3));
        assert!(!reservation.inflight);
        assert_eq!(state.inflight_requests(), 2);
        assert_eq!(state.pending_requests.len(), 3);
    }

    #[test]
    fn test_requests_stop_at_the_read_ahead_limit_above_the_tip() {
        let mut state = make_test_state();
        state.config = crate::Config::default()
            .with_parallel_requests(2)
            .with_batch_size(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let mut next_id = 0;

        let peer = PeerId::random();

        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(1), HeightStartType::Start),
            &mut next_id,
        );

        // Two batches fill the budget, covering heights 1 to 10.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 100)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![
                Height::new(1)..=Height::new(5),
                Height::new(6)..=Height::new(10)
            ]
        );

        // Both responses arrive in full, so no request is in flight any more.
        for (request_id, start) in [("req1", 1u64), ("req2", 6)] {
            drive_input_numbering_requests(
                &mut state,
                &metrics,
                Input::ValueResponse(
                    OutboundRequestId::new(request_id),
                    peer,
                    Some(crate::ValueResponse::new(
                        Height::new(start),
                        (start..=start + 4).map(make_raw_value).collect(),
                    )),
                ),
                &mut next_id,
            );
        }
        assert_eq!(state.inflight_requests(), 0);

        // The budget is free, but height 11 is above the highest permitted
        // request start, `tip + parallel_requests * batch_size` (10).
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 100)),
            &mut next_id,
        );
        assert!(
            requested_ranges(&effects).is_empty(),
            "no request is expected above the read-ahead limit, got {:?}",
            requested_ranges(&effects)
        );
        assert_eq!(state.pending_requests.len(), 2);

        // The limit moves up with the tip.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Decided(Height::new(5)),
            &mut next_id,
        );
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 100)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(11)..=Height::new(15)]
        );
    }

    #[test]
    fn test_a_decision_below_the_tip_keeps_the_read_ahead_limit() {
        let mut state = make_test_state();
        state.config = crate::Config::default()
            .with_parallel_requests(1)
            .with_batch_size(1);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let mut next_id = 0;

        let peer = PeerId::random();

        // Decide heights 1 and 2 with no peer around, then move on to height 3.
        for height in [1u64, 2] {
            drive_input_numbering_requests(
                &mut state,
                &metrics,
                Input::StartedHeight(Height::new(height), HeightStartType::Start),
                &mut next_id,
            );
            drive_input_numbering_requests(
                &mut state,
                &metrics,
                Input::Decided(Height::new(height)),
                &mut next_id,
            );
        }
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(3), HeightStartType::Start),
            &mut next_id,
        );
        assert_eq!(state.tip_height, Height::new(2));
        assert_eq!(state.sync_height, Height::new(3));

        // The decision for height 1 arrives after the one for height 2.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Decided(Height::new(1)),
            &mut next_id,
        );
        assert_eq!(state.tip_height, Height::new(2));

        // A peer reaches height 3, which is still inside the read-ahead limit.
        let effects = drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 3)),
            &mut next_id,
        );
        assert_eq!(
            requested_ranges(&effects),
            vec![Height::new(3)..=Height::new(3)]
        );
    }

    #[test]
    fn test_status_requests_a_gap_below_reservations_that_fill_the_map() {
        let mut state = make_test_state();
        state.started = true;
        state.config = crate::Config::default()
            .with_parallel_requests(2)
            .with_batch_size(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        let peer = PeerId::random();

        // Both slots hold a reservation, and height 1 below them is uncovered.
        // The read-ahead limit is tip + 10, so the gap is well inside it and
        // only the capacity check decides whether the request goes out.
        state.tip_height = Height::new(0);
        state.sync_height = Height::new(1);
        for (id, height) in [("res_a", 2u64), ("res_b", 3)] {
            state.pending_requests.insert(
                OutboundRequestId::new(id),
                PendingRequestEntry {
                    range: Height::new(height)..=Height::new(height),
                    peer,
                    excluded_peers: BTreeSet::new(),
                    inflight: false,
                },
            );
        }
        assert_eq!(state.pending_requests.len(), state.max_parallel_requests());
        assert_eq!(state.inflight_requests(), 0);

        let effects =
            drive_input_with_retries(&mut state, &metrics, Input::Status(status(peer, 100)))
                .unwrap();

        // The gap goes out first. The freed second slot then goes to the next
        // uncovered range above the reservations.
        let requested = requested_ranges(&effects);
        assert_eq!(
            requested.first(),
            Some(&(Height::new(1)..=Height::new(1))),
            "the gap below the reservations was not requested first: {requested:?}"
        );
    }

    #[test]
    fn test_re_request_respects_the_parallel_request_budget() {
        let mut state = make_test_state();
        state.config = crate::Config::default()
            .with_parallel_requests(2)
            .with_batch_size(5);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));
        let mut next_id = 0;
        let peer = PeerId::random();

        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(1), HeightStartType::Start),
            &mut next_id,
        );
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::Status(status(peer, 100)),
            &mut next_id,
        );

        // A one-value response leaves a one-height reservation. The freed slot
        // goes to the range just above it, so the budget is full again. A larger
        // reservation would leave the read-ahead limit as the binding gate, and
        // the budget would never fill.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::ValueResponse(
                OutboundRequestId::new("req1"),
                peer,
                Some(crate::ValueResponse::new(
                    Height::new(1),
                    vec![make_raw_value(1)],
                )),
            ),
            &mut next_id,
        );
        assert_eq!(state.inflight_requests(), 2);

        // A height inside the reservation fails locally. Re-requesting it must
        // not add a third request.
        drive_input_numbering_requests(
            &mut state,
            &metrics,
            Input::LocalTransientError(Height::new(1)),
            &mut next_id,
        );

        assert_eq!(
            state.inflight_requests(),
            2,
            "in-flight requests exceeded parallel_requests: {:?}",
            state
                .pending_requests
                .values()
                .map(|entry| format!("{}", DisplayRange(&entry.range)))
                .collect::<Vec<_>>()
        );

        // The range is uncovered again and the frontier points at it, so the
        // next request pass picks it up.
        assert_eq!(state.sync_height, Height::new(1));
        assert!(!state
            .pending_requests
            .values()
            .any(|entry| entry.range.contains(&Height::new(1))));
    }

    #[test]
    fn test_stale_partial_response_does_not_block_next_uncovered_range() {
        let mut state = make_test_state();
        state.started = true;
        state.consensus_height = Height::new(1);
        state.tip_height = Height::new(0);
        state.sync_height = Height::new(26);
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        let peer = PeerId::random();
        state.peers.insert(
            peer,
            crate::Status {
                peer_id: peer,
                tip_height: Height::new(25),
                history_min_height: Height::new(1),
            },
        );

        for (index, start) in [1, 6, 11, 16, 21].into_iter().enumerate() {
            state.pending_requests.insert(
                OutboundRequestId::new(format!("req{index}")),
                PendingRequestEntry {
                    range: Height::new(start)..=Height::new(start + 4),
                    peer,
                    excluded_peers: BTreeSet::new(),
                    inflight: true,
                },
            );
        }

        drive_input(
            &mut state,
            &metrics,
            Input::StartedHeight(Height::new(2), HeightStartType::Start),
        )
        .unwrap();

        drive_input(
            &mut state,
            &metrics,
            Input::Status(crate::Status {
                peer_id: peer,
                tip_height: Height::new(1),
                history_min_height: Height::new(1),
            }),
        )
        .unwrap();

        drive_input(
            &mut state,
            &metrics,
            Input::ValueResponse(
                OutboundRequestId::new("req0"),
                peer,
                Some(crate::ValueResponse::new(
                    Height::new(1),
                    vec![make_raw_value(1)],
                )),
            ),
        )
        .unwrap();

        for (index, start) in [6, 11, 16, 21].into_iter().enumerate() {
            let values = (start..=start + 4).map(make_raw_value).collect();
            drive_input(
                &mut state,
                &metrics,
                Input::ValueResponse(
                    OutboundRequestId::new(format!("req{}", index + 1)),
                    peer,
                    Some(crate::ValueResponse::new(Height::new(start), values)),
                ),
            )
            .unwrap();
        }

        let effects = drive_input_with_retries(
            &mut state,
            &metrics,
            Input::Status(crate::Status {
                peer_id: peer,
                tip_height: Height::new(25),
                history_min_height: Height::new(1),
            }),
        )
        .unwrap();

        assert!(
            effects.iter().any(|effect| matches!(
                effect,
                Effect::SendValueRequest(_, request, _)
                    if request.range == (Height::new(2)..=Height::new(5))
            )),
            "the next request cycle must request the uncovered range 2..=5"
        );
    }

    #[test]
    fn test_request_values_range_rolls_back_sync_height_on_send_failure() {
        let mut state = make_test_state();
        state.started = true;
        let metrics = crate::Metrics::new(std::time::Duration::from_secs(10));

        state.tip_height = Height::new(10);
        state.sync_height = Height::new(16);

        let peer = PeerId::random();

        state.peers.insert(
            peer,
            crate::Status {
                peer_id: peer,
                tip_height: Height::new(20),
                history_min_height: Height::new(1),
            },
        );

        // Pending request assigned to the peer for heights 11..=15.
        state.pending_requests.insert(
            OutboundRequestId::new("req1"),
            PendingRequestEntry {
                range: Height::new(11)..=Height::new(15),
                peer,
                excluded_peers: BTreeSet::new(),
                inflight: true,
            },
        );

        // Peer returns a partial response covering only heights 11..=12.
        let response = crate::ValueResponse::new(
            Height::new(11),
            vec![make_raw_value(11), make_raw_value(12)],
        );

        // A peer is available for the suffix but the send effect fails.
        drive_input_with_send_failures(
            &mut state,
            &metrics,
            Input::ValueResponse(OutboundRequestId::new("req1"), peer, Some(response)),
        )
        .unwrap();

        // sync_height must roll back to the suffix start (13) so the suffix
        // is retried on the next request cycle.
        assert_eq!(state.sync_height, Height::new(13));

        // No pending request covers the suffix start.
        for entry in state.pending_requests.values() {
            assert!(
                !entry.range.contains(&Height::new(13)),
                "No pending request should cover the un-retried suffix start",
            );
        }
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
        use malachitebft_core_types::ExtendedCommitCertificate;
        RawDecidedValue {
            value_bytes: Bytes::new(),
            certificate: ExtendedCommitCertificate {
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
                inflight: true,
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
                        Effect::CancelValueRequest(_, r) => r.resume_with(()),
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
}

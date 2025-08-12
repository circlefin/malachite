use std::cmp::max;
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
            // **NOTE** We cannot simply call `re_request_values_from_peer_except` here.
            // Although we received some values from the peer, these values have not yet been processed
            // by the consensus engine. If we called `re_request_values_from_peer_except`, we would
            // end up re-requesting the entire original range (including values we already received),
            // causing the syncing peer to repeatedly send multiple requests until the already-received
            // values are fully processed.
            // To tackle this, we first update the current pending request with the range of values
            // it provides we received, and then issue a new request with the remaining values.
            let new_start = requested_range.start().increment_by(response.values.len() as u64);
            let end = *requested_range.end();

            if response.values.is_empty() {
                error!(%request_id, %peer_id, "Received response contains no values");
            } else {
                // the response of this request only provided `response.values.len()` values, so update the pending request accordingly
                let updated_range = *requested_range.start()..=new_start.decrement().unwrap();
                state.pending_requests.insert(request_id.clone(), (updated_range, peer_id));
            }

            // issue a new request to the same peer for the remaining values
            let new_range = new_start..=end;
            request_values_from_peer(&co, state, metrics, new_range, peer_id).await?;
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
        warn!(
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
        // Build the next range of heights to request from a peer.
        let start_height = state.sync_height;
        let batch_size = max(1, state.config.batch_size as u64);
        let end_height = start_height.increment_by(batch_size - 1);
        let range = start_height..=end_height;

        // Get a random peer that can provide the values in the range.
        let Some((peer, range)) = state.random_peer_with(&range) else {
            debug!("No peer to request sync from");
            // No connected peer reached this height yet, we can stop syncing here.
            break;
        };

        request_values_from_peer(&co, state, metrics, range, peer).await?;
    }

    Ok(())
}

async fn request_values_from_peer<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    range: RangeInclusive<Ctx::Height>,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(range = %DisplayRange::<Ctx>(&range), peer.id = %peer, "Requesting sync from peer");

    if range.is_empty() {
        warn!(range.sync = %DisplayRange::<Ctx>(&range), %peer, "Range is empty, skipping request");
        return Ok(());
    }

    // Skip over any heights in the range that are not waiting for a response
    // (meaning that they have been validated by consensus or a peer).
    let range = state.trim_validated_heights(&range);
    if range.is_empty() {
        warn!(%peer, "All values in range {} have been validated, skipping request", DisplayRange::<Ctx>(&range));
        return Ok(());
    }

    // Send request to peer
    let Some(request_id) = perform!(
        co,
        Effect::SendValueRequest(peer, ValueRequest::new(range.clone()), Default::default()),
        Resume::ValueRequestId(id) => id,
    ) else {
        warn!(range = %DisplayRange::<Ctx>(&range), %peer, "Failed to send sync request to peer");
        return Ok(());
    };

    metrics.value_request_sent(range.start().as_u64());

    // Store pending request and move the sync height.
    debug!(%request_id, range = %DisplayRange::<Ctx>(&range), %peer, "Sent sync request to peer");
    state.sync_height = max(state.sync_height, range.end().increment());
    state.pending_requests.insert(request_id, (range, peer));

    Ok(())
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

    // It is possible that a prefix or the whole range of values has been validated via consensus.
    // Then, request only the missing values.
    let range = state.trim_validated_heights(&range);

    if range.is_empty() {
        warn!(
            %request_id,
            "All values in range {} have been validated, skipping re-request",
            DisplayRange::<Ctx>(&range)
        );

        return Ok(());
    }

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
        error!(
            range.sync = %DisplayRange::<Ctx>(&range),
            "No peer to request sync from"
        );

        return Ok(());
    };

    request_values_from_peer(&co, state, metrics, peer_range, peer).await?;

    Ok(())
}

struct DisplayRange<'a, Ctx: Context>(&'a RangeInclusive<Ctx::Height>);

impl<'a, Ctx: Context> core::fmt::Display for DisplayRange<'a, Ctx> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..={}", self.0.start(), self.0.end())
    }
}

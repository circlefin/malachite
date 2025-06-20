use derive_where::derive_where;
use tracing::{debug, error, info, warn};

use malachitebft_core_types::{Context, Height};

use crate::co::Co;
use crate::scoring::SyncResult;
use crate::{
    perform, Effect, Error, InboundRequestId, Metrics, OutboundRequestId, PeerId, RawDecidedValue,
    Request, Resume, State, Status, ValueRequest, ValueResponse,
};

#[derive_where(Debug)]
pub enum Input<Ctx: Context> {
    /// A tick has occurred
    Tick,

    /// A status update has been received from a peer
    Status(Status<Ctx>),

    /// Consensus just started a new height.
    /// The boolean indicates whether this was a restart or a new start.
    StartedHeight(Ctx::Height, bool),

    /// Consensus just decided on a new value
    Decided(Ctx::Height),

    /// A ValueSync request has been received from a peer
    ValueRequest(InboundRequestId, PeerId, ValueRequest<Ctx>),

    /// A (possibly empty or invalid) ValueSync response has been received
    ValueResponse(OutboundRequestId, PeerId, Option<ValueResponse<Ctx>>),

    /// Got a response from the application to our `GetValue` request
    GotDecidedValue(InboundRequestId, Ctx::Height, Option<RawDecidedValue<Ctx>>),

    /// A request for a value timed out
    SyncRequestTimedOut(PeerId, Request<Ctx>),

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

        Input::Decided(height) => on_decided(co, state, metrics, height).await,

        Input::ValueRequest(request_id, peer_id, request) => {
            on_value_request(co, state, metrics, request_id, peer_id, request).await
        }

        Input::ValueResponse(request_id, peer_id, Some(response)) => {
            on_value_response(co, state, metrics, request_id, peer_id, response).await
        }

        Input::ValueResponse(request_id, peer_id, None) => {
            on_invalid_value_response(co, state, metrics, request_id, peer_id).await
        }

        Input::GotDecidedValue(request_id, height, value) => {
            on_got_decided_value(co, state, metrics, request_id, height, value).await
        }

        Input::SyncRequestTimedOut(peer_id, request) => {
            on_sync_request_timed_out(co, state, metrics, peer_id, request).await
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
    debug!(%status.peer_id, %status.tip_height, "Received peer status");

    let peer_height = status.tip_height;

    state.update_status(status);

    if !state.started {
        // Consensus has not started yet, no need to sync (yet).
        return Ok(());
    }

    if peer_height > state.tip_height {
        warn!(
            height.tip = %state.tip_height,
            height.sync = %state.sync_height,
            height.peer = %peer_height,
            "SYNC REQUIRED: Falling behind"
        );

        // We are lagging behind one of our peer at least,
        // request sync from any peer already at or above that peer's height.
        request_values(co, state, metrics).await?;
    }

    Ok(())
}

pub async fn on_started_height<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    restart: bool,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let tip_height = height.decrement().unwrap_or(Height::ZERO);

    debug!(height.tip = %tip_height, height.sync = %height, %restart, "Starting new height");

    state.started = true;
    state.sync_height = height;
    state.tip_height = tip_height;

    // Trigger potential requests if possible.
    request_values(co, state, metrics).await?;

    Ok(())
}

pub async fn on_decided<Ctx>(
    _co: Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(height.tip = %height, "Updating tip height");

    state.tip_height = height;

    state.remove_pending_value_validation(&height);
    state.remove_pending_value_request_by_height(&height);

    Ok(())
}

pub async fn on_value_request<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    peer: PeerId,
    request: ValueRequest<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%request.height, %peer, "Received request for value");

    metrics.value_request_received(request.height.as_u64());

    perform!(
        co,
        Effect::GetDecidedValue(request_id, request.height, Default::default())
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
    debug!(%response.height, %request_id, %peer_id, "Received response");

    state.remove_pending_value_request_by_height(&response.height);

    let response_time = metrics.value_response_received(response.height.as_u64());

    if let Some(response_time) = response_time {
        let sync_result = response
            .value
            .as_ref()
            .map_or(SyncResult::Failure, |_| SyncResult::Success(response_time));

        state
            .peer_scorer
            .update_score_with_metrics(peer_id, sync_result, &metrics.scoring);
    }

    // We do not update the peer score if we do not know the response time.
    // This should never happen, but we need to handle it gracefully just in case.

    if response.value.is_none() {
        warn!(%response.height, %request_id, "Received invalid value response");

        // If we received an empty response, we will try to request the value from another peer.
        request_value_from_peer_except(co, state, metrics, response.height, peer_id).await?;
    } else {
        // TODO: handle the case where this event is received after the corresponding value decision event.
        state.store_pending_value_validation(response.height);
    }

    Ok(())
}

pub async fn on_invalid_value_response<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%request_id, %peer, "Received invalid response");

    if let Some(height) = state.remove_pending_value_request_by_id(&request_id) {
        debug!(%height, %request_id, "Found which height this request was for");

        // If we have an associated height for this request, we will try again and request it from another peer.
        request_value_from_peer_except(co, state, metrics, height, peer).await?;
    }

    Ok(())
}

pub async fn on_got_decided_value<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    height: Ctx::Height,
    value: Option<RawDecidedValue<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let response = match value {
        None => {
            error!(%height, "Received empty value response from host");
            None
        }
        Some(value) if value.certificate.height != height => {
            error!(
                %height, value.height = %value.certificate.height,
                "Received value response for wrong height from host"
            );
            None
        }
        Some(value) => {
            info!(%height, "Received value response from host, sending it out");
            Some(value)
        }
    };

    perform!(
        co,
        Effect::SendValueResponse(
            request_id,
            ValueResponse::new(height, response),
            Default::default()
        )
    );

    metrics.value_response_sent(height.as_u64());

    Ok(())
}

pub async fn on_sync_request_timed_out<Ctx>(
    _co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    request: Request<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match request {
        Request::ValueRequest(value_request) => {
            let height = value_request.height;
            warn!(%peer_id, %height, "Value request timed out");

            metrics.value_request_timed_out(height.as_u64());

            state.remove_pending_value_request_by_height(&height);
            state.peer_scorer.update_score(peer_id, SyncResult::Timeout);
        }
    };

    Ok(())
}

async fn on_invalid_value<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    from: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%from, %height, "Received invalid value");

    state.peer_scorer.update_score(from, SyncResult::Failure);

    state.remove_pending_value_validation(&height);
    state.remove_pending_value_request_by_height(&height);

    request_value_from_peer_except(co, state, metrics, height, from).await
}

async fn on_value_processing_error<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer: PeerId,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%peer, height.sync = %height, "Error while processing value");

    state.remove_pending_value_validation(&height);
    state.remove_pending_value_request_by_height(&height);

    // NOTE: We do not update the peer score here, as this is an internal error
    //       and not a failure from the peer's side.

    request_values(co, state, metrics).await
}

/// Requests values from heights in the current sync window. A request is sent for
/// a given height if there is no pending request or validation for that height.
async fn request_values<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let mut height = state.sync_height;

    let limit = state
        .sync_height
        .increment_by(state.config.parallel_requests);

    // Find out the first height for which we do not have a pending request or validation.
    while state.has_pending_value_request(&height) || state.has_pending_value_validation(&height) {
        height = height.increment();
        if height >= limit {
            break;
        }
    }

    // If the height we are trying to request is already above the sync height,
    // it means we already have a pending request or validation for the heights below.
    if height > state.sync_height {
        debug!(height.sync = %DisplayRange(state.sync_height, height), "Already have a pending request or validation for these heights");
    }

    // Start requesting values from the first height that does not have a pending request or validation.
    loop {
        if height >= limit {
            break;
        }

        let Some(peer) = state.random_peer_with_tip_at_or_above(height) else {
            debug!(height.sync = %height, "No peer to request sync from");
            // No peer reached this height yet, we can stop here.
            break;
        };

        request_value_from_peer(&co, state, metrics, height, peer).await?;

        height = height.increment();
    }

    Ok(())
}

async fn request_value_from_peer<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(height.sync = %height, %peer, "Requesting sync from peer");

    let request_id = perform!(
        co,
        Effect::SendValueRequest(peer, ValueRequest::new(height), Default::default()),
        Resume::ValueRequestId(id) => id,
    );

    metrics.value_request_sent(height.as_u64());

    if let Some(request_id) = request_id {
        debug!(%request_id, %peer, "Sent value request to peer");
        state.store_pending_value_request(height, request_id);
    } else {
        warn!(height.sync = %height, %peer, "Failed to send value request to peer");
    }

    Ok(())
}

async fn request_value_from_peer_except<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    except: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(height.sync = %height, "Requesting sync from another peer");

    state.remove_pending_value_request_by_height(&height);

    if let Some(peer) = state.random_peer_with_tip_at_or_above_except(height, except) {
        request_value_from_peer(&co, state, metrics, height, peer).await?;
    } else {
        error!(height.sync = %height, "No peer to request sync from");
    }

    Ok(())
}

struct DisplayRange<A>(A, A);

impl<A: core::fmt::Display> core::fmt::Display for DisplayRange<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..{}", self.0, self.1)
    }
}

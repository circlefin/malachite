use core::marker::PhantomData;

use derive_where::derive_where;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use malachitebft_core_types::{CertificateError, CommitCertificate, Context, Height};

use crate::co::Co;
use crate::{
    perform, InboundRequestId, Metrics, OutboundRequestId, PeerId, RawDecidedValue, Request, State,
    Status, ValueRequest, ValueResponse,
};

#[derive_where(Debug)]
#[derive(Error)]
pub enum Error<Ctx: Context> {
    /// The coroutine was resumed with a value which
    /// does not match the expected type of resume value.
    #[error("Unexpected resume: {0:?}, expected one of: {1}")]
    UnexpectedResume(Resume<Ctx>, &'static str),
}

#[derive_where(Debug)]
pub enum Resume<Ctx: Context> {
    Continue(PhantomData<Ctx>),
}

impl<Ctx: Context> Default for Resume<Ctx> {
    fn default() -> Self {
        Self::Continue(PhantomData)
    }
}

#[derive_where(Debug)]
pub enum Effect<Ctx: Context> {
    /// Broadcast our status to our direct peers
    BroadcastStatus(Ctx::Height),

    /// Send a ValueSync request to a peer
    SendValueRequest(PeerId, ValueRequest<Ctx>),

    /// Send a response to a ValueSync request
    SendValueResponse(InboundRequestId, ValueResponse<Ctx>),

    /// Retrieve a value from the application
    GetDecidedValue(InboundRequestId, Ctx::Height),
}

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

    /// A ValueSync response has been received
    ValueResponse(OutboundRequestId, PeerId, ValueResponse<Ctx>),

    /// Got a response from the application to our `GetValue` request
    GotDecidedValue(InboundRequestId, Ctx::Height, Option<RawDecidedValue<Ctx>>),

    /// A request for a value timed out
    SyncRequestTimedOut(PeerId, Request<Ctx>),

    /// We received an invalid [`CommitCertificate`]
    InvalidCertificate(PeerId, CommitCertificate<Ctx>, CertificateError<Ctx>),
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

        Input::ValueResponse(request_id, peer_id, response) => {
            on_value_response(co, state, metrics, request_id, peer_id, response).await
        }

        Input::GotDecidedValue(request_id, height, value) => {
            on_value(co, state, metrics, request_id, height, value).await
        }

        Input::SyncRequestTimedOut(peer_id, request) => {
            on_sync_request_timed_out(co, state, metrics, peer_id, request).await
        }

        Input::InvalidCertificate(peer, certificate, error) => {
            on_invalid_certificate(co, state, metrics, peer, certificate, error).await
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

    perform!(co, Effect::BroadcastStatus(state.tip_height));

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
        request_value(co, state, metrics).await?;
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
    let tip_height = height.decrement().unwrap_or(height);

    debug!(height.tip = %tip_height, height.sync = %height, %restart, "Starting new height");

    state.started = true;
    state.sync_height = height;
    state.tip_height = tip_height;

    // Check if there is any peer already at or above the height we just started,
    // and request sync from that peer in order to catch up.
    request_value(co, state, metrics).await?;

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
    state.remove_pending_decided_value_request(height);

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

    metrics.decided_value_request_received(request.height.as_u64());

    perform!(co, Effect::GetDecidedValue(request_id, request.height));

    Ok(())
}

pub async fn on_value_response<Ctx>(
    _co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer: PeerId,
    response: ValueResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%response.height, %request_id, %peer, "Received response");

    state.remove_pending_decided_value_request(response.height);

    metrics.decided_value_response_received(response.height.as_u64());

    Ok(())
}

pub async fn on_value<Ctx>(
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
                "Received value response for wrong height"
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
        Effect::SendValueResponse(request_id, ValueResponse::new(height, response))
    );

    metrics.decided_value_response_sent(height.as_u64());

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
            state.remove_pending_decided_value_request(height);
            metrics.decided_value_request_timed_out(height.as_u64());
        }
    };

    Ok(())
}

/// If there are no pending requests for the sync height,
/// and there is peer at a higher height than our sync height,
/// then sync from that peer.
async fn request_value<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let sync_height = state.sync_height;

    if state.has_pending_decided_value_request(&sync_height) {
        warn!(height.sync = %sync_height, "Already have a pending value request for this height");
        return Ok(());
    }

    if let Some(peer) = state.random_peer_with_tip_at_or_above(sync_height) {
        request_value_from_peer(co, state, metrics, sync_height, peer).await?;
    } else {
        debug!(height.sync = %sync_height, "No peer to request sync from");
    }

    Ok(())
}

async fn request_value_from_peer<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(height.sync = %height, %peer, "Requesting sync value from peer");

    perform!(
        co,
        Effect::SendValueRequest(peer, ValueRequest::new(height))
    );

    metrics.decided_value_request_sent(height.as_u64());
    state.store_pending_decided_value_request(height, peer);

    Ok(())
}

async fn on_invalid_certificate<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    from: PeerId,
    certificate: CommitCertificate<Ctx>,
    error: CertificateError<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    error!(%error, %certificate.height, %certificate.round, "Received invalid certificate");
    trace!("Certificate: {certificate:#?}");

    info!(height.sync = %certificate.height, "Requesting sync from another peer");
    state.remove_pending_decided_value_request(certificate.height);

    let Some(peer) = state.random_peer_with_tip_at_or_above_except(certificate.height, from) else {
        error!(height.sync = %certificate.height, "No other peer to request sync from");
        return Ok(());
    };

    request_value_from_peer(co, state, metrics, certificate.height, peer).await
}

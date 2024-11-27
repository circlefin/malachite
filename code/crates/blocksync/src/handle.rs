use core::marker::PhantomData;

use derive_where::derive_where;
use libp2p::request_response::OutboundRequestId;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

use malachite_common::{CertificateError, CommitCertificate, Context, Height, Round, VoteSet};

use crate::co::Co;
use crate::{perform, Request, VoteSetRequest, VoteSetResponse};
use crate::{
    BlockRequest, BlockResponse, InboundRequestId, Metrics, PeerId, State, Status, SyncedBlock,
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

    /// Send a BlockSync request to a peer
    SendBlockRequest(PeerId, BlockRequest<Ctx>),

    /// Send a response to a BlockSync request
    SendBlockResponse(InboundRequestId, BlockResponse<Ctx>),

    /// Retrieve a block from the application
    GetBlock(InboundRequestId, Ctx::Height),

    /// Send a VoteSet request to a peer
    SendVoteSetRequest(PeerId, VoteSetRequest<Ctx>),

    /// Send a response to a VoteSet request
    SendVoteSetResponse(InboundRequestId, VoteSetResponse<Ctx>),
}

#[derive_where(Debug)]
pub enum Input<Ctx: Context> {
    /// A tick has occurred
    Tick,

    /// A status update has been received from a peer
    Status(Status<Ctx>),

    /// Consensus just started a new height
    StartHeight(Ctx::Height),

    /// Consensus just decided on a new block
    UpdateHeight(Ctx::Height),

    /// A BlockSync request has been received from a peer
    BlockRequest(InboundRequestId, PeerId, BlockRequest<Ctx>),

    /// A BlockSync response has been received
    BlockResponse(OutboundRequestId, PeerId, BlockResponse<Ctx>),

    /// Got a response from the application to our `GetBlock` request
    GotBlock(InboundRequestId, Ctx::Height, Option<SyncedBlock<Ctx>>),

    /// A request for a block timed out
    SyncRequestTimedOut(PeerId, Request<Ctx>),

    /// We received an invalid [`CommitCertificate`]
    InvalidCertificate(PeerId, CommitCertificate<Ctx>, CertificateError<Ctx>),

    /// Get the vote set for the height at or above round
    GetVoteSet(Ctx::Height, Round),

    /// A VoteSet response has been received
    VoteSetResponse(OutboundRequestId, PeerId, VoteSetResponse<Ctx>),

    /// Got a response from consensus for our `GetVoteSet` request
    GotVoteSet(InboundRequestId, VoteSet<Ctx>),
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
        Input::StartHeight(height) => on_start_height(co, state, metrics, height).await,
        Input::UpdateHeight(height) => on_update_height(co, state, metrics, height).await,
        Input::BlockRequest(request_id, peer_id, request) => {
            on_block_request(co, state, metrics, request_id, peer_id, request).await
        }
        Input::BlockResponse(request_id, peer_id, response) => {
            on_block_response(co, state, metrics, request_id, peer_id, response).await
        }
        Input::GotBlock(request_id, height, block) => {
            on_block(co, state, metrics, request_id, height, block).await
        }
        Input::SyncRequestTimedOut(peer_id, request) => {
            on_sync_request_timed_out(co, state, metrics, peer_id, request).await
        }
        Input::InvalidCertificate(peer, certificate, error) => {
            on_invalid_certificate(co, state, metrics, peer, certificate, error).await
        }
        Input::GetVoteSet(height, round) => {
            on_get_vote_set(co, state, metrics, height, round).await
        }
        Input::VoteSetResponse(request_id, peer_id, response) => {
            on_vote_set_response(co, state, metrics, request_id, peer_id, response).await
        }
        Input::GotVoteSet(request_id, local_vote_set) => {
            on_vote_set(co, state, metrics, request_id, local_vote_set).await
        }
    }
}

#[tracing::instrument(skip_all)]
pub async fn on_tick<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(height = %state.tip_height, "Broadcasting status");

    perform!(co, Effect::BroadcastStatus(state.tip_height));

    Ok(())
}

#[tracing::instrument(
    skip_all,
    fields(
        sync_height = %state.sync_height,
        tip_height = %state.tip_height
    )
)]
pub async fn on_status<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    status: Status<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%status.peer_id, %status.height, "Received peer status");

    let peer_height = status.height;

    state.update_status(status);

    if peer_height > state.tip_height {
        info!(
            tip.height = %state.tip_height,
            sync.height = %state.sync_height,
            peer.height = %peer_height,
            "SYNC REQUIRED: Falling behind"
        );

        // We are lagging behind one of our peer at least,
        // request sync from any peer already at or above that peer's height.
        request_block(co, state, metrics).await?;
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn on_block_request<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    peer: PeerId,
    request: BlockRequest<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(height = %request.height, %peer, "Received request for block");

    metrics.request_received(request.height.as_u64());

    perform!(co, Effect::GetBlock(request_id, request.height));

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn on_block_response<Ctx>(
    _co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: OutboundRequestId,
    peer: PeerId,
    response: BlockResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(height = %response.height, %request_id, %peer, "Received response");

    metrics.response_received(response.height.as_u64());

    Ok(())
}

pub async fn on_start_height<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%height, "Starting new height");

    state.sync_height = height;

    // Check if there is any peer already at or above the height we just started,
    // and request sync from that peer in order to catch up.
    request_block(co, state, metrics).await?;

    Ok(())
}

pub async fn on_update_height<Ctx>(
    _co: Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    height: Ctx::Height,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.tip_height < height {
        debug!(%height, "Update height");

        state.tip_height = height;
        state.remove_pending_request(height);
    }

    Ok(())
}

pub async fn on_block<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    metrics: &Metrics,
    request_id: InboundRequestId,
    height: Ctx::Height,
    block: Option<SyncedBlock<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let response = match block {
        None => {
            error!(%height, "Received empty response");
            None
        }
        Some(block) if block.certificate.height != height => {
            error!(
                %height, block.height = %block.certificate.height,
                "Received block for wrong height"
            );
            None
        }
        Some(block) => {
            debug!(%height, "Received decided block");
            Some(block)
        }
    };

    perform!(
        co,
        Effect::SendBlockResponse(request_id, BlockResponse::new(height, response))
    );

    metrics.response_sent(height.as_u64());

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
    let height = match request {
        Request::BlockRequest(block_request) => block_request.height,
        Request::VoteSetRequest(vote_set_request) => vote_set_request.height,
    };
    warn!(%peer_id, %height, "Request timed out");

    metrics.request_timed_out(height.as_u64());

    state.remove_pending_request(height);

    Ok(())
}

/// If there are no pending requests for the sync height,
/// and there is peer at a higher height than our sync height,
/// then sync from that peer.
async fn request_block<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let sync_height = state.sync_height;

    if state.has_pending_request(&sync_height) {
        debug!(sync.height = %sync_height, "Already have a pending request for this height");
        return Ok(());
    }

    if let Some(peer) = state.random_peer_with_block(sync_height) {
        request_block_from_peer(co, state, metrics, sync_height, peer).await?;
    }

    Ok(())
}

async fn request_block_from_peer<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(sync.height = %height, %peer, "Requesting block from peer");

    perform!(
        co,
        Effect::SendBlockRequest(peer, BlockRequest::new(height))
    );

    metrics.request_sent(height.as_u64());
    state.store_pending_request(height, peer);

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

    info!("Requesting sync from another peer");
    state.remove_pending_request(certificate.height);

    let Some(peer) = state.random_peer_with_block_except(certificate.height, from) else {
        error!("No other peer to request sync from");
        return Ok(());
    };

    request_block_from_peer(co, state, metrics, certificate.height, peer).await
}

pub async fn on_get_vote_set<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    height: Ctx::Height,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    if state.has_pending_request(&height) {
        debug!(vote_set.height = %height, "Already have a pending vote set request for this height");
        return Ok(());
    }

    debug!(
        "VS4 - send vote set request to peer, number of peers {}",
        state.peers.len()
    );

    if let Some(peer) = state.random_peer_for_votes() {
        request_vote_set_from_peer(co, state, metrics, height, round, peer).await?;
    }
    Ok(())
}

async fn request_vote_set_from_peer<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    height: Ctx::Height,
    round: Round,
    peer: PeerId,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(vote_set.height = %height, vote_set.round = %round, %peer, "Requesting vote set from peer");

    perform!(
        co,
        Effect::SendVoteSetRequest(peer, VoteSetRequest::new(height, round))
    );

    // TODO - metrics
    //metrics.request_sent(height.as_u64());

    // TODO - vote request has round, currently it conflicts with block reqs, they should co-exist for same height
    state.store_pending_request(height, peer);

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn on_vote_set_response<Ctx>(
    _co: Co<Ctx>,
    _state: &mut State<Ctx>,
    _metrics: &Metrics,
    request_id: OutboundRequestId,
    peer: PeerId,
    _response: VoteSetResponse<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%request_id, %peer, "Received vote set response");

    Ok(())
}

pub async fn on_vote_set_request_timed_out<Ctx>(
    _co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    peer_id: PeerId,
    request: VoteSetRequest<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    warn!(%peer_id, %request.height, "Vote set request timed out");

    metrics.request_timed_out(request.height.as_u64());

    state.remove_pending_request(request.height);

    Ok(())
}

pub async fn on_vote_set<Ctx>(
    co: Co<Ctx>,
    _state: &mut State<Ctx>,
    _metrics: &Metrics,
    request_id: InboundRequestId,
    vote_set: VoteSet<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    perform!(
        co,
        Effect::SendVoteSetResponse(request_id, VoteSetResponse::new(vote_set))
    );

    Ok(())
}

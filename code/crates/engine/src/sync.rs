use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use bytesize::ByteSize;
use derive_where::derive_where;
use eyre::eyre;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::SeedableRng;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn, Instrument};

use malachitebft_codec as codec;
use malachitebft_core_consensus::util::bounded_queue::BoundedQueue;
use malachitebft_core_consensus::PeerId;
use malachitebft_core_types::utils::height::DisplayRange;
use malachitebft_core_types::ValueResponse as CoreValueResponse;
use malachitebft_core_types::{Context, ExtendedCommitCertificate};
use malachitebft_network::Channel;
use malachitebft_sync::{
    self as sync, HeightStartType, InboundFailureReason, InboundRequestId, OutboundRequestId,
    RawDecidedValue, Request, Response, Resumable,
};

use crate::consensus::{ConsensusMsg, ConsensusRef};
use crate::host::{HostMsg, HostRef};
use crate::network::{NetworkEvent, NetworkMsg, NetworkRef, Status};
use crate::util::ticker::ticker;
use crate::util::timers::{TimeoutElapsed, TimerScheduler};

/// Codec for sync protocol messages
///
/// This trait is automatically implemented for any type that implements:
/// - [`codec::Codec<sync::Status<Ctx>>`]
/// - [`codec::Codec<sync::Request<Ctx>>`]
/// - [`codec::Codec<sync::Response<Ctx>>`]
pub trait SyncCodec<Ctx>
where
    Ctx: Context,
    Self: codec::Codec<sync::Status<Ctx>>,
    Self: codec::Codec<sync::Request<Ctx>>,
    Self: codec::Codec<sync::Response<Ctx>>,
    Self: codec::HasEncodedLen<sync::Response<Ctx>>,
{
}

impl<Ctx, Codec> SyncCodec<Ctx> for Codec
where
    Ctx: Context,
    Codec: codec::Codec<sync::Status<Ctx>>,
    Codec: codec::Codec<sync::Request<Ctx>>,
    Codec: codec::Codec<sync::Response<Ctx>>,
    Codec: codec::HasEncodedLen<sync::Response<Ctx>>,
{
}

#[derive_where(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Timeout<Ctx: Context> {
    /// Timeout for an outbound sync request.
    Request(OutboundRequestId),

    /// Budget for an inbound sync request: caps how long a pending inbound
    /// request waits on the host before it is dropped.
    InboundRequest(InboundRequestId),

    /// Backoff before re-requesting a synced value at the given height after a
    /// local/transient processing failure.
    Retry(Ctx::Height),
}

type Timers<Ctx> = TimerScheduler<Timeout<Ctx>>;

/// Base delay for the exponential backoff applied before re-requesting a synced
/// value after a local/transient processing failure.
const LOCAL_TRANSIENT_RETRY_BASE: Duration = Duration::from_millis(100);

/// Cap for the exponential backoff applied before re-requesting a synced value
/// after a local/transient processing failure.
const LOCAL_TRANSIENT_RETRY_CAP: Duration = Duration::from_secs(1);

/// Capped exponential backoff before re-requesting a synced value after a
/// local/transient processing failure: `min(BASE * 2^(attempt - 1), CAP)`.
fn local_transient_retry_delay(attempt: u32) -> Duration {
    // Clamp the shift exponent so it cannot overflow (2^16 fits in u32).
    let shift = attempt.saturating_sub(1).min(16);
    let factor = 1u32 << shift;
    LOCAL_TRANSIENT_RETRY_BASE
        .saturating_mul(factor)
        .min(LOCAL_TRANSIENT_RETRY_CAP)
}

pub type SyncRef<Ctx> = ActorRef<Msg<Ctx>>;
pub type SyncMsg<Ctx> = Msg<Ctx>;

#[derive_where(Clone, Debug)]
pub struct RawDecidedBlock<Ctx: Context> {
    pub height: Ctx::Height,
    pub certificate: ExtendedCommitCertificate<Ctx>,
    pub value_bytes: Bytes,
}

#[derive_where(Clone, Debug)]
pub struct InflightRequest<Ctx: Context> {
    pub peer_id: PeerId,
    pub request_id: OutboundRequestId,
    pub request: Request<Ctx>,
}

pub type InflightRequests<Ctx> = HashMap<OutboundRequestId, InflightRequest<Ctx>>;

/// Pending inbound sync requests and the peer that issued each.
pub type InboundRequests = HashMap<InboundRequestId, PeerId>;

#[derive_where(Clone, Debug)]
pub enum Msg<Ctx: Context> {
    /// Internal tick
    Tick,

    /// Receive an even from gossip layer
    NetworkEvent(NetworkEvent<Ctx>),

    /// Consensus has decided on a value at the given height
    Decided(Ctx::Height),

    /// Consensus has (re)started a new height.
    ///
    /// The second argument indicates whether this is a restart or not.
    StartedHeight(Ctx::Height, HeightStartType),

    /// Host has a response for the blocks request
    GotDecidedValues(
        InboundRequestId,
        RangeInclusive<Ctx::Height>,
        Vec<RawDecidedValue<Ctx>>,
    ),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout<Ctx>>),

    /// A fault in a synced value (its certificate or its bytes) is attributable
    /// to the peer that served it: penalize and re-request from another peer.
    PeerFault(PeerId, Ctx::Height),

    /// Processing a synced value hit a local/transient failure (e.g. the
    /// execution layer being temporarily unavailable). No peer is to blame, so
    /// no peer is carried — re-request without penalizing or excluding anyone.
    LocalTransientError(Ctx::Height),
}

impl<Ctx: Context> From<NetworkEvent<Ctx>> for Msg<Ctx> {
    fn from(event: NetworkEvent<Ctx>) -> Self {
        Msg::NetworkEvent(event)
    }
}

impl<Ctx: Context> From<TimeoutElapsed<Timeout<Ctx>>> for Msg<Ctx> {
    fn from(elapsed: TimeoutElapsed<Timeout<Ctx>>) -> Self {
        Msg::TimeoutElapsed(elapsed)
    }
}

#[derive(Debug)]
pub struct Params {
    /// Interval at which to update other peers of our status
    /// If set to 0s, status updates are sent eagerly right after each decision.
    /// Default: 5s
    pub status_update_interval: Duration,

    /// Timeout duration for sync requests
    /// Default: 10s
    pub request_timeout: Duration,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            status_update_interval: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// A sync value buffered in the queue, tagged with the request that produced it.
#[derive_where(Clone, Debug)]
struct BufferedValue<Ctx: Context> {
    request_id: OutboundRequestId,
    value: CoreValueResponse<Ctx>,
}

impl<Ctx: Context> BufferedValue<Ctx> {
    fn new(request_id: OutboundRequestId, value: CoreValueResponse<Ctx>) -> Self {
        Self { request_id, value }
    }
}

/// A queue of buffered sync values for heights ahead of consensus, keyed by height.
type SyncQueue<Ctx> = BoundedQueue<<Ctx as Context>::Height, BufferedValue<Ctx>>;

fn sync_queue_capacity(config: &sync::Config) -> usize {
    let read_ahead_window = config.read_ahead_window();
    let capacity = read_ahead_window.saturating_mul(2);
    let max_requested_heights =
        read_ahead_window.saturating_add(config.effective_batch_size().saturating_sub(1));

    debug_assert!(capacity >= max_requested_heights);
    capacity
}

/// The mode for sending status updates
enum StatusUpdateMode {
    /// Send status updates at regular intervals
    Interval(JoinHandle<()>), // the ticker task handle

    /// Send status updates eagerly before starting the next height
    Eager,
}

pub struct State<Ctx: Context> {
    /// The state of the sync state machine
    sync: sync::State<Ctx>,

    /// Scheduler for timers
    timers: Timers<Ctx>,

    /// Per-height count of consecutive local/transient processing failures,
    /// used to compute the exponential backoff before re-requesting.
    local_transient_attempts: HashMap<Ctx::Height, u32>,

    /// In-flight requests
    inflight: InflightRequests<Ctx>,

    /// Pending inbound requests and the peer that issued each.
    inbound: InboundRequests,

    /// Queue of sync value responses for heights ahead of consensus
    sync_queue: SyncQueue<Ctx>,

    /// Status update mode
    status_update_mode: StatusUpdateMode,
}

struct HandlerState<'a, Ctx: Context> {
    /// Scheduler for timers, used to start new timers for outgoing requests
    /// and correlate elapsed timers to the original request and peer.
    timers: &'a mut Timers<Ctx>,
    /// In-flight requests, used to correlate timeouts and responses to the original request and peer.
    inflight: &'a mut InflightRequests<Ctx>,
    /// Pending inbound requests, used to stop tracking a request once it is answered.
    inbound: &'a mut InboundRequests,
    /// Buffer for sync responses for heights ahead of consensus, keyed by height.
    sync_queue: &'a mut SyncQueue<Ctx>,
    /// The current consensus height according to the last processed input.
    consensus_height: Ctx::Height,
}

#[allow(dead_code)]
pub struct Sync<Ctx, Codec>
where
    Ctx: Context,
    Codec: SyncCodec<Ctx>,
{
    ctx: Ctx,
    network: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    consensus: ConsensusRef<Ctx>,
    params: Params,
    sync_codec: Codec,
    sync_config: sync::Config,
    metrics: sync::Metrics,
    span: tracing::Span,
}

impl<Ctx, Codec> Sync<Ctx, Codec>
where
    Ctx: Context,
    Codec: SyncCodec<Ctx>,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx,
        network: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        consensus: ConsensusRef<Ctx>,
        params: Params,
        sync_codec: Codec,
        sync_config: sync::Config,
        metrics: sync::Metrics,
        span: tracing::Span,
    ) -> Self {
        Self {
            ctx,
            network,
            host,
            consensus,
            params,
            sync_codec,
            sync_config,
            metrics,
            span,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        network: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        consensus: ConsensusRef<Ctx>,
        params: Params,
        sync_codec: Codec,
        sync_config: sync::Config,
        metrics: sync::Metrics,
        span: tracing::Span,
    ) -> Result<SyncRef<Ctx>, ractor::SpawnErr> {
        let actor = Self::new(
            ctx,
            network,
            host,
            consensus,
            params,
            sync_codec,
            sync_config,
            metrics,
            span,
        );
        let (actor_ref, _) = Actor::spawn(None, actor, ()).await?;
        Ok(actor_ref)
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: sync::Input<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let mut handler_state = HandlerState {
            timers: &mut state.timers,
            inflight: &mut state.inflight,
            inbound: &mut state.inbound,
            sync_queue: &mut state.sync_queue,
            consensus_height: state.sync.consensus_height,
        };

        malachitebft_sync::process!(
            input: input,
            state: &mut state.sync,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(
                    myself,
                    &mut handler_state,
                    effect,
                ).await
            }
        )
    }

    async fn get_history_min_height(&self) -> Result<Ctx::Height, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::GetHistoryMinHeight {
            reply_to
        })
        .map_err(|e| eyre!("Failed to get earliest history height: {e:?}").into())
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut HandlerState<'_, Ctx>,
        effect: sync::Effect<Ctx>,
    ) -> Result<sync::Resume<Ctx>, ActorProcessingErr> {
        use sync::Effect;

        match effect {
            Effect::BroadcastStatus(height, r) => {
                let history_min_height = self.get_history_min_height().await?;

                self.network.cast(NetworkMsg::BroadcastStatus(Status::new(
                    height,
                    history_min_height,
                )))?;

                Ok(r.resume_with(()))
            }

            Effect::SendValueRequest(peer_id, value_request, r) => {
                let request = Request::ValueRequest(value_request);
                let result = ractor::call!(self.network, |reply_to| {
                    NetworkMsg::OutgoingRequest(peer_id, request.clone(), reply_to)
                });

                match result {
                    Ok(request_id) => {
                        let request_id = OutboundRequestId::new(request_id);

                        state.timers.start_timer(
                            Timeout::Request(request_id.clone()),
                            self.params.request_timeout,
                        );

                        state.inflight.insert(
                            request_id.clone(),
                            InflightRequest {
                                peer_id,
                                request_id: request_id.clone(),
                                request,
                            },
                        );

                        info!(%peer_id, %request_id, "Sent value request to peer");

                        Ok(r.resume_with(Some(request_id)))
                    }
                    Err(e) => {
                        error!("Failed to send request to network layer: {e}");
                        Ok(r.resume_with(None))
                    }
                }
            }

            Effect::SendValueResponse(request_id, value_response, r) => {
                // The inbound request is being answered: stop tracking it and
                // cancel its stall timer before handing the response to the
                // network layer.
                state
                    .timers
                    .cancel(&Timeout::InboundRequest(request_id.clone()));
                state.inbound.remove(&request_id);

                let response = Response::ValueResponse(value_response);
                self.network
                    .cast(NetworkMsg::OutgoingResponse(request_id, response))?;

                Ok(r.resume_with(()))
            }

            Effect::GetDecidedValues(request_id, range, r) => {
                self.host.call_and_forward(
                    {
                        let range = range.clone();
                        |reply_to| HostMsg::GetDecidedValues { range, reply_to }
                    },
                    myself,
                    |values| Msg::<Ctx>::GotDecidedValues(request_id, range, values),
                    None,
                )?;

                Ok(r.resume_with(()))
            }

            Effect::ProcessValueResponse(peer_id, request_id, response, r) => {
                self.process_value_response(state, peer_id, request_id, response);
                Ok(r.resume_with(()))
            }

            Effect::CancelValueRequest(request_id, r) => {
                self.network.cast(NetworkMsg::CancelRequest(request_id))?;

                Ok(r.resume_with(()))
            }
        }
    }

    fn process_value_response(
        &self,
        state: &mut HandlerState<'_, Ctx>,
        peer_id: PeerId,
        request_id: OutboundRequestId,
        response: sync::ValueResponse<Ctx>,
    ) {
        let consensus_height = state.consensus_height;
        let mut ignored = Vec::new();
        let mut buffered = Vec::new();

        for raw_value in response.values {
            let height = raw_value.height();
            let value = raw_value.to_core(peer_id);

            match height.cmp(&consensus_height) {
                // The value is for a height that has already been decided, ignore it.
                Ordering::Less => {
                    ignored.push(height);
                }

                // The value is for a height ahead of consensus, buffer it for later processing when we reach that height.
                Ordering::Greater => {
                    let buffered_value = BufferedValue::new(request_id.clone(), value);
                    if state.sync_queue.push(height, buffered_value) {
                        buffered.push(height);
                    } else {
                        warn!(%peer_id, %request_id, %height, "Failed to buffer sync response, queue is full");
                    }
                }

                // The value is for the current consensus height, process it immediately.
                Ordering::Equal => {
                    debug!(%peer_id, %request_id, %height, "Processing value for current consensus height");

                    if let Err(e) = self
                        .consensus
                        .cast(ConsensusMsg::ProcessSyncResponse(value))
                    {
                        error!("Failed to forward value response to consensus: {e}");
                    }
                }
            }
        }

        self.metrics
            .sync_queue_updated(state.sync_queue.len(), state.sync_queue.size());

        if !ignored.is_empty() {
            debug!(
                %peer_id, %request_id, ?ignored,
                "Ignored {} values for already decided heights", ignored.len()
            );
        }

        if !buffered.is_empty() {
            debug!(
                %peer_id, %request_id, ?buffered,
                "Buffered {} values for heights ahead of consensus", buffered.len()
            );
        }
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Tick => {
                self.process_input(&myself, state, sync::Input::SendStatusUpdate)
                    .await?;
            }

            Msg::NetworkEvent(NetworkEvent::PeerDisconnected(peer_id)) => {
                info!(%peer_id, "Disconnected from peer");

                // Cancel timers and drop in-flight requests routed to this peer,
                // then let the sync state machine reissue them to another peer.
                let peer_request_ids: Vec<OutboundRequestId> = state
                    .inflight
                    .iter()
                    .filter(|(_, inflight)| inflight.peer_id == peer_id)
                    .map(|(request_id, _)| request_id.clone())
                    .collect();

                for request_id in &peer_request_ids {
                    state.timers.cancel(&Timeout::Request(request_id.clone()));
                    state.inflight.remove(request_id);
                }

                if !peer_request_ids.is_empty() {
                    debug!(
                        %peer_id,
                        count = peer_request_ids.len(),
                        "Cleared in-flight requests for disconnected peer",
                    );
                }

                // Drop any pending inbound requests issued by this peer: cancel
                // their stall timers and evict them from the network layer
                // before the host reply path runs.
                let inbound_request_ids =
                    drain_inbound_requests_for_peer(&mut state.inbound, peer_id);

                for request_id in &inbound_request_ids {
                    state
                        .timers
                        .cancel(&Timeout::InboundRequest(request_id.clone()));
                    self.network
                        .cast(NetworkMsg::CancelInboundRequest(request_id.clone()))?;
                    self.metrics.value_inbound_request_failed(
                        request_id,
                        InboundFailureReason::RequesterDisconnected,
                    );
                }

                if !inbound_request_ids.is_empty() {
                    debug!(
                        %peer_id,
                        count = inbound_request_ids.len(),
                        "Cleared pending inbound requests for disconnected peer",
                    );
                }

                self.process_input(&myself, state, sync::Input::PeerDisconnected(peer_id))
                    .await?;
            }

            Msg::NetworkEvent(NetworkEvent::Status(peer_id, status)) => {
                let status = sync::Status {
                    peer_id,
                    tip_height: status.tip_height,
                    history_min_height: status.history_min_height,
                };

                self.process_input(&myself, state, sync::Input::Status(status))
                    .await?;
            }

            Msg::NetworkEvent(NetworkEvent::SyncRequest(request_id, from, request)) => {
                // Track the request against its requester and arm its stall timer.
                state.inbound.insert(request_id.clone(), from);
                state.timers.start_timer(
                    Timeout::InboundRequest(request_id.clone()),
                    self.params.request_timeout,
                );

                match request {
                    Request::ValueRequest(value_request) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::ValueRequest(request_id, from, value_request),
                        )
                        .await?;
                    }
                };
            }

            Msg::NetworkEvent(NetworkEvent::SyncResponse(request_id, peer, response)) => {
                // Cancel the timer associated with the request for which we just received a response
                state.timers.cancel(&Timeout::Request(request_id.clone()));

                // Remove the in-flight request
                if state.inflight.remove(&request_id).is_none() {
                    debug!(%request_id, %peer, "Received response for unknown request");

                    // Ignore response for unknown request
                    // This can happen if the request timed out and was removed from in-flight requests
                    // in the meantime or if we receive a duplicate response.
                    return Ok(());
                }

                let response = response.map(|resp| match resp {
                    Response::ValueResponse(value_response) => value_response,
                });

                self.process_input(
                    &myself,
                    state,
                    sync::Input::ValueResponse(request_id, peer, response),
                )
                .await?;
            }

            Msg::NetworkEvent(NetworkEvent::SyncRequestFailed(request_id, peer, reason)) => {
                state.timers.cancel(&Timeout::Request(request_id.clone()));

                let Some(inflight) = state.inflight.remove(&request_id) else {
                    // Request was already cleaned up (e.g. by an earlier
                    // `PeerDisconnected` for the same peer, or by the response
                    // arriving in a tight race with the failure event).
                    debug!(%request_id, %peer, ?reason, "Sync request failure for unknown request");
                    return Ok(());
                };

                self.process_input(
                    &myself,
                    state,
                    sync::Input::SyncRequestFailed(
                        request_id,
                        inflight.peer_id,
                        inflight.request,
                        reason,
                    ),
                )
                .await?;
            }

            Msg::NetworkEvent(NetworkEvent::PeerSubscribed(peer_id, Channel::Sync)) => {
                debug!(%peer_id, "Peer subscribed to sync channel, broadcasting status");

                self.process_input(&myself, state, sync::Input::SendStatusUpdate)
                    .await?;
            }

            Msg::NetworkEvent(_) => {
                // Ignore other gossip events
            }

            // (Re)Started a new height
            Msg::StartedHeight(height, restart) => {
                if restart.is_restart() {
                    // Clear the sync queue
                    state.sync_queue.clear();
                    self.metrics.sync_queue_updated(0, 0);
                }

                self.process_input(&myself, state, sync::Input::StartedHeight(height, restart))
                    .await?;

                // Drain buffered sync responses for this height
                for buffered in state.sync_queue.shift_and_take(&height) {
                    if let Err(e) = self
                        .consensus
                        .cast(ConsensusMsg::ProcessSyncResponse(buffered.value))
                    {
                        error!("Failed to forward buffered sync response to consensus: {e}");
                        break;
                    }
                }

                // Update metrics
                self.metrics
                    .sync_queue_heights
                    .set(state.sync_queue.len() as i64);
                self.metrics
                    .sync_queue_size
                    .set(state.sync_queue.size() as i64);
            }

            // Decided on a value
            Msg::Decided(height) => {
                self.process_input(&myself, state, sync::Input::Decided(height))
                    .await?;

                // Progress was made: drop the local/transient backoff state for any
                // height at or below the decided one and cancel its pending retry timer.
                let reset: Vec<Ctx::Height> = state
                    .local_transient_attempts
                    .keys()
                    .filter(|h| **h <= height)
                    .copied()
                    .collect();

                for h in reset {
                    state.local_transient_attempts.remove(&h);
                    state.timers.cancel(&Timeout::Retry(h));
                }

                // In Eager mode, broadcast our status immediately after deciding
                // rather than waiting for the next height to start, so that peers
                // who need to sync from us learn about our latest height sooner.
                if let StatusUpdateMode::Eager = &state.status_update_mode {
                    self.process_input(&myself, state, sync::Input::SendStatusUpdate)
                        .await?;
                }
            }

            // Received decided values from host
            //
            // We need to ensure that the total size of the response does not exceed the maximum allowed size.
            // If it does, we truncate the response accordingly.
            // This is to prevent sending overly large messages that could lead to network issues.
            Msg::GotDecidedValues(request_id, range, mut values) => {
                // Drop late host replies for inbound requests that were already
                // evicted (requester disconnected, or the stall timer fired).
                if !state.inbound.contains_key(&request_id) {
                    debug!(%request_id, "Dropping decided values for evicted inbound request");
                    return Ok(());
                }

                debug!(
                    %request_id,
                    range = %DisplayRange(&range),
                    values_count = values.len(),
                    "Processing decided values from host"
                );

                // Filter values to respect maximum response size
                let max_response_size = ByteSize::b(self.sync_config.max_response_size as u64);
                truncate_values_to_size_limit(&mut values, max_response_size, &self.sync_codec);

                self.process_input(
                    &myself,
                    state,
                    sync::Input::GotDecidedValues(request_id, range, values),
                )
                .await?;
            }

            Msg::PeerFault(peer, height) => {
                // Remove buffered values that came from the same request as the faulty value.
                // This prevents stale values from a bad peer from being drained to consensus
                // when the height advances.
                if let Some((request_id, _)) = state.sync.get_request_id_by(height) {
                    let removed = state.sync_queue.retain(|_, bv| bv.request_id != request_id);

                    if removed > 0 {
                        debug!(
                            %peer, %height, %request_id, removed,
                            "Removed buffered values from invalidated request"
                        );
                        self.metrics
                            .sync_queue_updated(state.sync_queue.len(), state.sync_queue.size());
                    }
                }

                self.process_input(&myself, state, sync::Input::PeerFault(peer, height))
                    .await?
            }

            Msg::LocalTransientError(height) => {
                // Count the transient error when it is first observed, not when the
                // backoff retry later fires: the Retry timer can be cancelled (e.g. the
                // height is decided via another peer during backoff), which would
                // otherwise drop the error from the metric.
                self.metrics.value_local_transient_error();

                // Do not re-request immediately: during a multi-minute execution-layer
                // outage an immediate re-request becomes a tight loop. Back off with a
                // capped exponential delay and let the retry timer fire the re-request.
                let attempt = state.local_transient_attempts.entry(height).or_insert(0);
                *attempt = attempt.saturating_add(1);
                let attempt = *attempt;

                let delay = local_transient_retry_delay(attempt);

                debug!(%height, attempt, ?delay, "Backing off before re-requesting synced value after local/transient error");

                state.timers.start_timer(Timeout::Retry(height), delay);
            }

            Msg::TimeoutElapsed(elapsed) => {
                let Some(timeout) = state.timers.intercept_timer_msg(elapsed) else {
                    // Timer was cancelled or already processed, ignore
                    return Ok(());
                };

                info!(?timeout, "Timeout elapsed");

                match timeout {
                    Timeout::Request(request_id) => {
                        if let Some(inflight) = state.inflight.remove(&request_id) {
                            self.process_input(
                                &myself,
                                state,
                                sync::Input::SyncRequestTimedOut(
                                    request_id,
                                    inflight.peer_id,
                                    inflight.request,
                                ),
                            )
                            .await?;
                        } else {
                            debug!(%request_id, "Timeout for unknown request");
                        }
                    }

                    // The host did not answer within the inbound request budget:
                    // drop it from the network layer and record the reason.
                    Timeout::InboundRequest(request_id) => {
                        if state.inbound.remove(&request_id).is_some() {
                            self.network
                                .cast(NetworkMsg::CancelInboundRequest(request_id.clone()))?;
                            self.metrics.value_inbound_request_failed(
                                &request_id,
                                InboundFailureReason::HostStallTimeout,
                            );
                            debug!(%request_id, "Inbound sync request timed out waiting on host");
                        } else {
                            debug!(%request_id, "Inbound request timeout for unknown request");
                        }
                    }

                    // The backoff after a local/transient error has elapsed:
                    // now re-request the synced value (without penalizing any peer).
                    // The retry attempt was already counted when the engine
                    // actor received `Msg::LocalTransientError`.
                    Timeout::Retry(height) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::LocalTransientError(height),
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn status_update_mode<Ctx, R>(
    interval: Duration,
    sync: &ActorRef<Msg<Ctx>>,
    rng: &mut R,
) -> StatusUpdateMode
where
    Ctx: Context,
    R: rand::Rng,
{
    if interval == Duration::ZERO {
        info!("Using status update mode: Eager");
        StatusUpdateMode::Eager
    } else {
        info!("Using status update mode: Interval");

        // One-time uniform adjustment factor [-1%, +1%]
        const ADJ_RATE: f64 = 0.01;
        let adjustment = rng.gen_range(-ADJ_RATE..=ADJ_RATE);

        let ticker = tokio::spawn(
            ticker(interval, sync.clone(), adjustment, || Msg::Tick).in_current_span(),
        );

        StatusUpdateMode::Interval(ticker)
    }
}

fn truncate_values_to_size_limit<Ctx, Codec>(
    values: &mut Vec<RawDecidedValue<Ctx>>,
    max_response_size: ByteSize,
    codec: &Codec,
) where
    Ctx: Context,
    Codec: SyncCodec<Ctx>,
{
    let mut current_size = ByteSize::b(0);
    let mut keep_count = 0;

    for value in values.iter() {
        let height = value.certificate.height;

        let value_response =
            Response::ValueResponse(sync::ValueResponse::new(height, vec![value.clone()]));

        let value_size = match codec.encoded_len(&value_response) {
            Ok(size) => ByteSize::b(size as u64),
            Err(e) => {
                error!("Failed to get response size for value, stopping at height {height}: {e}");
                break;
            }
        };

        if current_size + value_size > max_response_size {
            warn!(
                %max_response_size, %current_size, %value_size,
                "Maximum size limit would be exceeded, stopping at height {height}"
            );
            break;
        }

        current_size += value_size;
        keep_count += 1;
    }

    // Drop the remaining elements past the size limit
    values.truncate(keep_count);
}

/// Remove and return the IDs of all pending inbound requests issued by
/// `peer_id`.
fn drain_inbound_requests_for_peer(
    inbound: &mut InboundRequests,
    peer_id: PeerId,
) -> Vec<InboundRequestId> {
    let mut request_ids = Vec::new();

    inbound.retain(|request_id, requester| {
        if *requester == peer_id {
            request_ids.push(request_id.clone());
            false
        } else {
            true
        }
    });

    request_ids
}

#[async_trait]
impl<Ctx, Codec> Actor for Sync<Ctx, Codec>
where
    Ctx: Context,
    Codec: SyncCodec<Ctx>,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        self.network
            .cast(NetworkMsg::Subscribe(Box::new(myself.clone())))?;

        let mut rng = Box::new(rand::rngs::StdRng::from_entropy());

        let status_update_mode =
            status_update_mode(self.params.status_update_interval, &myself, &mut rng);

        // A batch may start at the end of the read-ahead window and extend by
        // one batch less one height. Twice the window covers that full range.
        let queue_capacity = sync_queue_capacity(&self.sync_config);

        Ok(State {
            sync: sync::State::new(rng, self.sync_config),
            timers: Timers::new(Box::new(myself.clone())),
            local_transient_attempts: HashMap::new(),
            inflight: HashMap::new(),
            inbound: HashMap::new(),
            sync_queue: SyncQueue::new(queue_capacity, queue_capacity),
            status_update_mode,
        })
    }

    #[tracing::instrument(
        name = "sync",
        parent = &self.span,
        skip_all,
        fields(
            tip_height = %state.sync.tip_height,
            sync_height = %state.sync.sync_height,
        ),
    )]
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(e) = self.handle_msg(myself, msg, state).await {
            error!("Error handling message: {e:?}");
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let StatusUpdateMode::Interval(ticker) = &state.status_update_mode {
            ticker.abort();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_queue_capacity_covers_read_ahead_ranges() {
        for parallel_requests in 0..=8 {
            for batch_size in 0..=8 {
                let config = sync::Config::default()
                    .with_parallel_requests(parallel_requests)
                    .with_batch_size(batch_size);
                let max_requested_heights = config
                    .read_ahead_window()
                    .saturating_add(config.effective_batch_size().saturating_sub(1));
                let queue_capacity = sync_queue_capacity(&config);

                assert!(
                    queue_capacity >= max_requested_heights,
                    "queue capacity {queue_capacity} is smaller than the read-ahead range \
                     {max_requested_heights} for parallel_requests={parallel_requests}, \
                     batch_size={batch_size}"
                );
            }
        }
    }

    #[test]
    fn drain_inbound_requests_for_peer_evicts_only_that_peers_requests() {
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        let mut inbound = InboundRequests::new();
        inbound.insert(InboundRequestId::new("a1"), peer_a);
        inbound.insert(InboundRequestId::new("a2"), peer_a);
        inbound.insert(InboundRequestId::new("b1"), peer_b);

        let mut evicted = drain_inbound_requests_for_peer(&mut inbound, peer_a);
        evicted.sort();

        assert_eq!(
            evicted,
            vec![InboundRequestId::new("a1"), InboundRequestId::new("a2")]
        );
        // Only the disconnected peer's requests are removed.
        assert_eq!(inbound.len(), 1);
        assert_eq!(inbound.get(&InboundRequestId::new("b1")), Some(&peer_b));
    }

    #[test]
    fn drain_inbound_requests_for_peer_is_noop_for_unknown_peer() {
        let peer_a = PeerId::random();
        let unknown = PeerId::random();

        let mut inbound = InboundRequests::new();
        inbound.insert(InboundRequestId::new("a1"), peer_a);

        let evicted = drain_inbound_requests_for_peer(&mut inbound, unknown);

        assert!(evicted.is_empty());
        assert_eq!(inbound.len(), 1);
    }
}

use std::cmp::{max, Ordering};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use bytesize::ByteSize;
use derive_where::derive_where;
use eyre::eyre;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::{Rng, SeedableRng};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::{AbortHandle, JoinHandle};
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

/// State for a pending inbound sync request: the peer that issued it and,
/// once the host-call task has been spawned, an `AbortHandle` for cancelling
/// the task on eviction so its admission and execution permits release
/// without waiting on `request_timeout`.
pub struct InboundRequest {
    pub peer_id: PeerId,
    pub abort_handle: Option<AbortHandle>,
}

impl InboundRequest {
    fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            abort_handle: None,
        }
    }

    fn abort_task(&mut self) {
        if let Some(handle) = self.abort_handle.take() {
            handle.abort();
        }
    }
}

/// Pending inbound sync requests keyed by request id.
pub type InboundRequests = HashMap<InboundRequestId, InboundRequest>;

#[derive_where(Clone, Debug)]
pub enum Msg<Ctx: Context> {
    /// Internal tick
    Tick,

    /// Internal periodic trigger for a value-sync request pass
    RetrySync,

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
    /// The request id is the one that delivered the faulty value.
    PeerFault(PeerId, Ctx::Height, OutboundRequestId),

    /// Processing a synced value hit a local/transient failure (e.g. the
    /// execution layer being temporarily unavailable). No peer is to blame, so
    /// no peer is carried — re-request without penalizing or excluding anyone.
    LocalTransientError(Ctx::Height),

    /// Periodic tick to prune stale entries from the inbound rate limiter.
    /// Fires regardless of [`Params::status_update_interval`] so the keyed
    /// limiter's per-peer state stays bounded in `Eager` status-update mode.
    PruneInboundRateLimiter,
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
#[derive(Clone, Debug)]
struct BufferedValue<V> {
    request_id: OutboundRequestId,
    value: V,
}

impl<V> BufferedValue<V> {
    fn new(request_id: OutboundRequestId, value: V) -> Self {
        Self { request_id, value }
    }
}

/// A queue of buffered sync values for heights ahead of consensus, keyed by height.
type SyncQueue<Ctx> = BoundedQueue<<Ctx as Context>::Height, BufferedValue<CoreValueResponse<Ctx>>>;

/// Drop everything still buffered from the request that served a faulty value.
fn purge_values_from_request<H: Ord, V>(
    sync_queue: &mut BoundedQueue<H, BufferedValue<V>>,
    request_id: &OutboundRequestId,
) -> usize {
    sync_queue.retain(|_, buffered| &buffered.request_id != request_id)
}

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

    /// Background ticker that periodically starts a value-sync request pass.
    /// Independent from the status-update ticker, so its cadence follows
    /// `request_timeout` rather than `status_update_interval`.
    retry_ticker: JoinHandle<()>,

    /// Background ticker that periodically prunes the inbound rate limiter.
    /// Independent from the status-update ticker so pruning runs even when
    /// `status_update_interval == 0` (Eager mode).
    rate_limiter_pruner: JoinHandle<()>,

    /// Admission permits: capacity for `max_concurrent + max_pending`.
    inbound_admission_permits: Arc<Semaphore>,

    /// Execution permits: capacity for `max_concurrent` concurrent host calls.
    inbound_execution_permits: Arc<Semaphore>,
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
    /// Lowest height this node will serve.
    history_min_height: Ctx::Height,
    /// Admission-permits handle (concurrent + pending capacity).
    inbound_admission_permits: Arc<Semaphore>,
    /// Execution-permits handle (concurrent capacity), acquired inside spawned tasks.
    inbound_execution_permits: Arc<Semaphore>,
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
            history_min_height: state.sync.history_min_height,
            inbound_admission_permits: state.inbound_admission_permits.clone(),
            inbound_execution_permits: state.inbound_execution_permits.clone(),
        };

        let result = async {
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
        .await;

        state.sync.history_min_height = handler_state.history_min_height;

        result
    }

    async fn get_history_min_height(&self) -> Result<Ctx::Height, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::GetHistoryMinHeight {
            reply_to
        })
        .map_err(|e| eyre!("Failed to get earliest history height: {e:?}").into())
    }

    /// Release a pending inbound request: abort its in-flight host call, cancel
    /// its stall timer, release the per-peer in-flight slot, drop the network
    /// layer's response channel, and record `reason`. Returns `true` if the
    /// request was pending.
    async fn evict_inbound_request(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        request_id: &InboundRequestId,
        reason: InboundFailureReason,
    ) -> Result<bool, ActorProcessingErr> {
        if !take_inbound_request(&mut state.inbound, request_id) {
            return Ok(false);
        }

        state
            .timers
            .cancel(&Timeout::InboundRequest(request_id.clone()));

        self.process_input(
            myself,
            state,
            sync::Input::InboundRequestEvicted(request_id.clone()),
        )
        .await?;

        self.network
            .cast(NetworkMsg::CancelInboundRequest(request_id.clone()))?;

        self.metrics
            .value_inbound_request_failed(request_id, reason);

        Ok(true)
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
                let history_min_height = match tokio::time::timeout(
                    self.params.request_timeout,
                    self.get_history_min_height(),
                )
                .await
                {
                    Ok(Ok(history_min_height)) => history_min_height,
                    Ok(Err(error)) => {
                        warn!(
                            ?error,
                            "Failed to get earliest history height, broadcasting status with last known floor"
                        );
                        state.history_min_height
                    }
                    Err(_) => {
                        warn!(
                            timeout = ?self.params.request_timeout,
                            "Timed out getting earliest history height, broadcasting status with last known floor"
                        );
                        state.history_min_height
                    }
                };

                // Host fetches can complete after the serve floor has advanced.
                // Never reopen a range that the host has already pruned.
                state.history_min_height = max(state.history_min_height, history_min_height);

                self.network.cast(NetworkMsg::BroadcastStatus(Status::new(
                    height,
                    state.history_min_height,
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
                let Some(admission_permit) =
                    try_acquire_request_permit(&state.inbound_admission_permits)
                else {
                    warn!(
                        %request_id,
                        range = %DisplayRange(&range),
                        max_concurrent = self.sync_config.max_concurrent_inbound_requests,
                        max_pending = self.sync_config.max_pending_inbound_requests,
                        "Rejecting inbound value request: admission capacity exhausted"
                    );
                    // Route the empty response through `GotDecidedValues` so the
                    // sync handle releases the per-peer in-flight slot it took
                    // at admission time.
                    myself.cast(Msg::<Ctx>::GotDecidedValues(request_id, range, Vec::new()))?;
                    return Ok(r.resume_with(()));
                };

                let execution_permits = state.inbound_execution_permits.clone();
                let host = self.host.clone();
                let actor_ref = myself.clone();
                let request_timeout = self.params.request_timeout;
                let spawn_request_id = request_id.clone();

                let join_handle = tokio::spawn(
                    async move {
                        let _admission = admission_permit;

                        let Ok(_execution) = execution_permits.acquire_owned().await else {
                            // Execution semaphore closed: actor shutting down.
                            // Cast an empty response so the sync handle releases
                            // the per-peer in-flight slot taken at admission
                            // time, matching the admission-rejection path.
                            let _ = actor_ref.cast(Msg::<Ctx>::GotDecidedValues(
                                spawn_request_id,
                                range,
                                Vec::new(),
                            ));
                            return;
                        };

                        let values = match host
                            .call(
                                |reply_to| HostMsg::GetDecidedValues {
                                    range: range.clone(),
                                    reply_to,
                                },
                                Some(request_timeout),
                            )
                            .await
                        {
                            Ok(ractor::rpc::CallResult::Success(values)) => values,
                            _ => Vec::new(),
                        };

                        let _ = actor_ref.cast(Msg::<Ctx>::GotDecidedValues(
                            spawn_request_id,
                            range,
                            values,
                        ));
                    }
                    .in_current_span(),
                );

                // Attach the task's abort handle so eviction paths can cancel
                // it and release the permit guards without waiting on the
                // ractor timeout.
                if let Some(entry) = state.inbound.get_mut(&request_id) {
                    entry.abort_handle = Some(join_handle.abort_handle());
                }

                Ok(r.resume_with(()))
            }

            Effect::ProcessValueResponse(peer_id, request_id, response, r) => {
                self.process_value_response(state, peer_id, request_id, response);
                Ok(r.resume_with(()))
            }

            Effect::CancelValueRequest(request_id, r) => {
                // Cancel the request timer and drop the in-flight entry.
                abandon_outbound_request(
                    state.timers,
                    Timeout::Request(request_id.clone()),
                    state.inflight,
                    &request_id,
                );
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
                        .cast(ConsensusMsg::ProcessSyncResponse(request_id.clone(), value))
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

            Msg::RetrySync => {
                self.process_input(&myself, state, sync::Input::TryRequestValues)
                    .await?;
            }

            Msg::PruneInboundRateLimiter => {
                state.sync.inbound_rate_limiter.retain_recent();
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

                // Drop any pending inbound requests issued by this peer before
                // the host reply path runs.
                let inbound_request_ids = inbound_request_ids_for_peer(&state.inbound, peer_id);

                for request_id in &inbound_request_ids {
                    // Every id came from `state.inbound`, so each is pending.
                    let _ = self
                        .evict_inbound_request(
                            &myself,
                            state,
                            request_id,
                            InboundFailureReason::RequesterDisconnected,
                        )
                        .await?;
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
                state
                    .inbound
                    .insert(request_id.clone(), InboundRequest::new(from));
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

            Msg::NetworkEvent(NetworkEvent::SyncInboundRequestFailed(request_id, peer)) => {
                if self
                    .evict_inbound_request(
                        &myself,
                        state,
                        &request_id,
                        InboundFailureReason::ConnectionClosed,
                    )
                    .await?
                {
                    debug!(%request_id, %peer, "Evicted inbound sync request whose connection closed");
                } else {
                    debug!(%request_id, %peer, "Inbound sync request failure for unknown request");
                }
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
                    if let Err(e) = self.consensus.cast(ConsensusMsg::ProcessSyncResponse(
                        buffered.request_id,
                        buffered.value,
                    )) {
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
                // Late reply for an evicted request: drop and release the slot.
                if !state.inbound.contains_key(&request_id) {
                    debug!(%request_id, "Dropping decided values for evicted inbound request");
                    self.process_input(
                        &myself,
                        state,
                        sync::Input::InboundRequestEvicted(request_id),
                    )
                    .await?;
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

            Msg::PeerFault(peer, height, request_id) => {
                // Drop the values still buffered from the request that supplied the faulty
                // one, so the height advance does not drain them to consensus. Requests
                // that have since taken over any of those heights keep their values.
                let removed = purge_values_from_request(&mut state.sync_queue, &request_id);

                if removed > 0 {
                    debug!(
                        %peer, %height, %request_id, removed,
                        "Removed buffered values from invalidated request"
                    );
                    self.metrics
                        .sync_queue_updated(state.sync_queue.len(), state.sync_queue.size());
                }

                self.process_input(
                    &myself,
                    state,
                    sync::Input::PeerFault(peer, height, request_id),
                )
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

                    // Host did not answer within the inbound request budget.
                    Timeout::InboundRequest(request_id) => {
                        if self
                            .evict_inbound_request(
                                &myself,
                                state,
                                &request_id,
                                InboundFailureReason::HostStallTimeout,
                            )
                            .await?
                        {
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

/// Cancel the outbound request timer and drop the in-flight entry.
///
/// Shared by [`Effect::CancelValueRequest`].
fn abandon_outbound_request<Key, V>(
    timers: &mut TimerScheduler<Key>,
    timeout_key: Key,
    inflight: &mut HashMap<OutboundRequestId, V>,
    request_id: &OutboundRequestId,
) where
    Key: Clone + Eq + Hash + Send + 'static,
{
    timers.cancel(&timeout_key);
    inflight.remove(request_id);
}

/// One-time uniform adjustment factor [-1%, +1%] applied to a ticker interval.
const TICKER_ADJ_RATE: f64 = 0.01;

/// Floor for the retry ticker. Its interval is derived from `request_timeout`,
/// which a config may set to `0`; `ticker` clamps a zero sleep to ~1ns and
/// would spin the actor in a hot message loop.
const MIN_RETRY_INTERVAL: Duration = Duration::from_secs(1);

/// This interval controls the fallback value-sync request pass. Explicit
/// failures retry immediately. A request with no response waits for this ticker.
fn retry_interval(request_timeout: Duration) -> Duration {
    request_timeout.max(MIN_RETRY_INTERVAL)
}

fn spawn_retry_ticker<Ctx, R>(
    request_timeout: Duration,
    sync: &ActorRef<Msg<Ctx>>,
    rng: &mut R,
) -> JoinHandle<()>
where
    Ctx: Context,
    R: rand::Rng,
{
    let interval = retry_interval(request_timeout);
    let adjustment = rng.gen_range(-TICKER_ADJ_RATE..=TICKER_ADJ_RATE);

    tokio::spawn(ticker(interval, sync.clone(), adjustment, || Msg::RetrySync).in_current_span())
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

        return StatusUpdateMode::Eager;
    }

    info!("Using status update mode: Interval");

    let adjustment = rng.gen_range(-TICKER_ADJ_RATE..=TICKER_ADJ_RATE);
    let ticker =
        tokio::spawn(ticker(interval, sync.clone(), adjustment, || Msg::Tick).in_current_span());

    StatusUpdateMode::Interval(ticker)
}

fn try_acquire_request_permit(permits: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    permits.clone().try_acquire_owned().ok()
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

/// Return the IDs of all pending inbound requests issued by `peer_id`.
fn inbound_request_ids_for_peer(
    inbound: &InboundRequests,
    peer_id: PeerId,
) -> Vec<InboundRequestId> {
    inbound
        .iter()
        .filter(|(_, entry)| entry.peer_id == peer_id)
        .map(|(request_id, _)| request_id.clone())
        .collect()
}

/// Remove a pending inbound request, aborting its host-call task if one has
/// been spawned. Returns `true` if the request was pending.
fn take_inbound_request(inbound: &mut InboundRequests, request_id: &InboundRequestId) -> bool {
    match inbound.remove(request_id) {
        Some(mut entry) => {
            entry.abort_task();
            true
        }
        None => false,
    }
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

        let retry_ticker = spawn_retry_ticker(self.params.request_timeout, &myself, &mut rng);

        // A batch may start at the end of the read-ahead window and extend by
        // one batch less one height. Twice the window covers that full range.
        let queue_capacity = sync_queue_capacity(&self.sync_config);

        let inbound_admission_permits = Arc::new(Semaphore::new(
            self.sync_config
                .max_concurrent_inbound_requests
                .saturating_add(self.sync_config.max_pending_inbound_requests),
        ));
        let inbound_execution_permits = Arc::new(Semaphore::new(
            self.sync_config.max_concurrent_inbound_requests,
        ));

        // Prune once per rate-limit window (governor only evicts entries whose
        // quota has fully replenished).
        let prune_interval = self.sync_config.inbound_request_rate_limit_window;
        let prune_adjustment = rng.gen_range(-TICKER_ADJ_RATE..=TICKER_ADJ_RATE);
        let rate_limiter_pruner = tokio::spawn(
            ticker(prune_interval, myself.clone(), prune_adjustment, || {
                Msg::PruneInboundRateLimiter
            })
            .in_current_span(),
        );

        Ok(State {
            sync: sync::State::new(rng, self.sync_config),
            timers: Timers::new(Box::new(myself.clone())),
            local_transient_attempts: HashMap::new(),
            inflight: HashMap::new(),
            inbound: HashMap::new(),
            sync_queue: SyncQueue::new(queue_capacity, queue_capacity),
            status_update_mode,
            retry_ticker,
            rate_limiter_pruner,
            inbound_admission_permits,
            inbound_execution_permits,
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

        state.retry_ticker.abort();
        state.rate_limiter_pruner.abort();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use malachitebft_test::codec::json::JsonCodec;
    use malachitebft_test::TestContext;

    use super::*;
    use tokio::sync::oneshot;

    struct IgnoreActor<Msg> {
        _marker: PhantomData<fn(Msg)>,
    }

    #[async_trait]
    impl<Msg> Actor for IgnoreActor<Msg>
    where
        Msg: ractor::Message,
    {
        type Msg = Msg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Msg>,
            _args: (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Msg>,
            _msg: Msg,
            _state: &mut (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }
    }

    async fn spawn_ignore_actor<Msg>() -> ActorRef<Msg>
    where
        Msg: ractor::Message,
    {
        IgnoreActor::<Msg>::spawn(
            None,
            IgnoreActor {
                _marker: PhantomData,
            },
            (),
        )
        .await
        .unwrap()
        .0
    }

    struct RetryObserver;

    #[async_trait]
    impl Actor for RetryObserver {
        type Msg = Msg<TestContext>;
        type State = Option<oneshot::Sender<()>>;
        type Arguments = oneshot::Sender<()>;

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            sender: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(Some(sender))
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            if matches!(msg, Msg::RetrySync) {
                if let Some(sender) = state.take() {
                    let _ = sender.send(());
                }
            }

            Ok(())
        }
    }

    type TestSyncQueue = BoundedQueue<u64, BufferedValue<&'static str>>;

    /// One peer served the range twice because a no-blame retry selected it again.
    fn queue_with_original_and_retry() -> TestSyncQueue {
        let mut queue = TestSyncQueue::new(16, 16);

        for height in [12, 13] {
            assert!(queue.push(
                height,
                BufferedValue::new(OutboundRequestId::new("original"), "original"),
            ));
            assert!(queue.push(
                height,
                BufferedValue::new(OutboundRequestId::new("retry"), "retry"),
            ));
        }

        queue
    }

    #[test]
    fn purging_the_originating_request_keeps_the_retry() {
        let mut queue = queue_with_original_and_retry();

        let removed = purge_values_from_request(&mut queue, &OutboundRequestId::new("original"));

        assert_eq!(removed, 2);

        assert_eq!(queue.size(), 2);
        for height in [12u64, 13] {
            let remaining: Vec<_> = queue.shift_and_take(&height).collect();
            assert_eq!(remaining.len(), 1);
            assert_eq!(remaining[0].request_id, OutboundRequestId::new("retry"));
        }
    }

    #[test]
    fn purging_an_unknown_request_keeps_the_queue() {
        let mut queue = queue_with_original_and_retry();

        let removed = purge_values_from_request(&mut queue, &OutboundRequestId::new("unknown"));

        assert_eq!(removed, 0);
        assert_eq!(queue.size(), 4);
    }

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
    fn inbound_request_ids_for_peer_lists_only_that_peers_requests() {
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        let mut inbound = InboundRequests::new();
        inbound.insert(InboundRequestId::new("a1"), InboundRequest::new(peer_a));
        inbound.insert(InboundRequestId::new("a2"), InboundRequest::new(peer_a));
        inbound.insert(InboundRequestId::new("b1"), InboundRequest::new(peer_b));

        let mut listed = inbound_request_ids_for_peer(&inbound, peer_a);
        listed.sort();

        assert_eq!(
            listed,
            vec![InboundRequestId::new("a1"), InboundRequestId::new("a2")]
        );
        // Listing leaves the map untouched; removal happens per request.
        assert_eq!(inbound.len(), 3);
    }

    #[test]
    fn inbound_request_ids_for_peer_is_empty_for_unknown_peer() {
        let peer_a = PeerId::random();
        let unknown = PeerId::random();

        let mut inbound = InboundRequests::new();
        inbound.insert(InboundRequestId::new("a1"), InboundRequest::new(peer_a));

        let listed = inbound_request_ids_for_peer(&inbound, unknown);

        assert!(listed.is_empty());
        assert_eq!(inbound.len(), 1);
    }

    #[test]
    fn take_inbound_request_reports_absent_request() {
        let mut inbound = InboundRequests::new();
        inbound.insert(
            InboundRequestId::new("r1"),
            InboundRequest::new(PeerId::random()),
        );

        assert!(!take_inbound_request(
            &mut inbound,
            &InboundRequestId::new("other")
        ));
        assert_eq!(inbound.len(), 1);

        assert!(take_inbound_request(
            &mut inbound,
            &InboundRequestId::new("r1")
        ));
        assert!(inbound.is_empty());
    }

    #[test]
    fn inbound_request_abort_task_cancels_the_spawned_task() {
        // Direct coverage of the helper `take_inbound_request` relies on.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let task = tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            });
            let mut entry = InboundRequest::new(PeerId::random());
            entry.abort_handle = Some(task.abort_handle());

            entry.abort_task();

            assert!(entry.abort_handle.is_none());
            let outcome = task.await;
            assert!(
                outcome.is_err() && outcome.err().unwrap().is_cancelled(),
                "abort_task must cancel the spawned task",
            );
        });
    }

    #[test]
    fn inbound_request_abort_task_is_noop_when_no_handle_attached() {
        let mut entry = InboundRequest::new(PeerId::random());
        entry.abort_task();
        assert!(entry.abort_handle.is_none());
    }

    #[test]
    fn take_inbound_request_aborts_the_spawned_task() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let peer = PeerId::random();
            let mut inbound = InboundRequests::new();

            let task = tokio::spawn(async {
                // Long enough that only abort will end this task.
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            });
            let abort_handle = task.abort_handle();
            let mut entry = InboundRequest::new(peer);
            entry.abort_handle = Some(abort_handle);
            inbound.insert(InboundRequestId::new("r1"), entry);

            assert!(take_inbound_request(
                &mut inbound,
                &InboundRequestId::new("r1")
            ));
            assert!(inbound.is_empty());

            let outcome = task.await;
            assert!(
                outcome.is_err() && outcome.err().unwrap().is_cancelled(),
                "the spawned task must be cancelled when the request is taken"
            );
        });
    }

    #[test]
    fn try_acquire_request_permit_returns_some_when_a_permit_is_available() {
        let permits = Arc::new(Semaphore::new(1));

        let permit = try_acquire_request_permit(&permits);

        assert!(permit.is_some());
        assert_eq!(permits.available_permits(), 0);
    }

    #[test]
    fn try_acquire_request_permit_returns_none_when_fully_saturated() {
        let permits = Arc::new(Semaphore::new(1));
        let _held = try_acquire_request_permit(&permits).expect("first permit");

        let second = try_acquire_request_permit(&permits);

        assert!(second.is_none());
    }

    #[test]
    fn try_acquire_request_permit_returns_none_for_zero_capacity() {
        let permits = Arc::new(Semaphore::new(0));

        assert!(try_acquire_request_permit(&permits).is_none());
    }

    #[test]
    fn the_retry_interval_follows_the_request_timeout() {
        assert_eq!(
            retry_interval(Duration::from_secs(10)),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn a_zero_request_timeout_is_floored_to_the_minimum_retry_interval() {
        assert_eq!(
            retry_interval(Duration::ZERO),
            MIN_RETRY_INTERVAL,
            "A zero interval would make `ticker` spin the actor in a hot loop"
        );
    }

    #[test]
    fn a_request_timeout_below_the_floor_is_raised_to_it() {
        assert_eq!(retry_interval(Duration::from_millis(1)), MIN_RETRY_INTERVAL);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_ticker_fires_in_both_status_update_modes() {
        let network = spawn_ignore_actor::<NetworkMsg<TestContext>>().await;
        let host = spawn_ignore_actor::<HostMsg<TestContext>>().await;
        let consensus = spawn_ignore_actor::<ConsensusMsg<TestContext>>().await;

        for status_update_interval in [Duration::ZERO, Duration::from_secs(60)] {
            let (sender, receiver) = oneshot::channel();
            let retry_observer = RetryObserver::spawn(None, RetryObserver, sender)
                .await
                .unwrap()
                .0;
            let actor = Sync::new(
                TestContext::new(),
                network.clone(),
                host.clone(),
                consensus.clone(),
                Params {
                    status_update_interval,
                    request_timeout: Duration::ZERO,
                },
                JsonCodec,
                sync::Config::default(),
                sync::Metrics::default(),
                tracing::Span::none(),
            );

            let mut state = actor.pre_start(retry_observer.clone(), ()).await.unwrap();

            tokio::task::yield_now().await;
            tokio::time::advance(MIN_RETRY_INTERVAL + Duration::from_millis(20)).await;

            let received = tokio::time::timeout(Duration::from_millis(1), receiver).await;
            assert!(
                matches!(received, Ok(Ok(()))),
                "retry ticker did not fire with status_update_interval={status_update_interval:?}"
            );

            actor
                .post_stop(retry_observer.clone(), &mut state)
                .await
                .unwrap();
            retry_observer.stop(None);
        }

        network.stop(None);
        host.stop(None);
        consensus.stop(None);
    }

    struct RecordStatus;

    #[async_trait]
    impl Actor for RecordStatus {
        type Msg = NetworkMsg<TestContext>;
        type State = Option<oneshot::Sender<Status<TestContext>>>;
        type Arguments = oneshot::Sender<Status<TestContext>>;

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            sender: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(Some(sender))
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            if let NetworkMsg::BroadcastStatus(status) = msg {
                if let Some(sender) = state.take() {
                    let _ = sender.send(status);
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn status_broadcast_falls_back_when_the_host_drops_the_history_fetch() {
        let (status_tx, status_rx) = oneshot::channel();
        let network = RecordStatus::spawn(None, RecordStatus, status_tx)
            .await
            .unwrap()
            .0;
        let host = spawn_ignore_actor::<HostMsg<TestContext>>().await;
        let consensus = spawn_ignore_actor::<ConsensusMsg<TestContext>>().await;

        let sync = Sync::spawn(
            TestContext::new(),
            network.clone(),
            host.clone(),
            consensus.clone(),
            Params {
                status_update_interval: Duration::ZERO,
                request_timeout: Duration::from_secs(30),
            },
            JsonCodec,
            sync::Config::default(),
            sync::Metrics::default(),
            tracing::Span::none(),
        )
        .await
        .unwrap();

        sync.cast(Msg::StartedHeight(
            malachitebft_test::Height::new(10),
            sync::HeightStartType::Start,
        ))
        .unwrap();

        let status = tokio::time::timeout(Duration::from_secs(2), status_rx)
            .await
            .expect("host dropped the history fetch and no status was broadcast")
            .expect("status recorder dropped");

        assert_eq!(status.tip_height, malachitebft_test::Height::new(9));
        assert_eq!(status.history_min_height, malachitebft_test::Height::new(0));

        sync.stop(None);
        network.stop(None);
        host.stop(None);
        consensus.stop(None);
    }

    #[test]
    fn try_acquire_request_permit_releases_the_permit_when_dropped() {
        let permits = Arc::new(Semaphore::new(1));

        {
            let _permit = try_acquire_request_permit(&permits).expect("permit");
            assert_eq!(permits.available_permits(), 0);
        }

        assert_eq!(permits.available_permits(), 1);
        assert!(try_acquire_request_permit(&permits).is_some());
    }

    /// Stand-in for [`Timeout::Request`]: the abandon helper is generic over the
    /// timer key so this module can cover timer/`inflight` bookkeeping without
    /// constructing a full [`Context`].
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct RequestTimeoutKey(OutboundRequestId);

    #[derive(Debug)]
    struct AbandonTimerMsg(#[allow(dead_code)] TimeoutElapsed<RequestTimeoutKey>);

    impl From<TimeoutElapsed<RequestTimeoutKey>> for AbandonTimerMsg {
        fn from(timer_msg: TimeoutElapsed<RequestTimeoutKey>) -> Self {
            AbandonTimerMsg(timer_msg)
        }
    }

    struct AbandonTimerActor;

    #[async_trait]
    impl Actor for AbandonTimerActor {
        type State = ();
        type Arguments = ();
        type Msg = AbandonTimerMsg;

        async fn pre_start(
            &self,
            _myself: ActorRef<AbandonTimerMsg>,
            _args: (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<AbandonTimerMsg>,
            _msg: AbandonTimerMsg,
            _state: &mut (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }
    }

    async fn abandon_timer_scheduler() -> TimerScheduler<RequestTimeoutKey> {
        let actor_ref = AbandonTimerActor::spawn(None, AbandonTimerActor, ())
            .await
            .unwrap()
            .0;
        TimerScheduler::new(Box::new(actor_ref))
    }

    #[tokio::test]
    async fn abandon_outbound_request_cancels_timer_and_removes_inflight() {
        let mut timers = abandon_timer_scheduler().await;
        let request_id = OutboundRequestId::new("req1");
        let key = RequestTimeoutKey(request_id.clone());

        timers.start_timer(key.clone(), Duration::from_secs(60));
        let mut inflight = HashMap::from([(request_id.clone(), "peer-a")]);

        abandon_outbound_request(&mut timers, key.clone(), &mut inflight, &request_id);

        assert!(
            !timers.is_timer_active(&key),
            "CancelValueRequest must cancel Timeout::Request"
        );
        assert!(
            !inflight.contains_key(&request_id),
            "CancelValueRequest must remove the inflight entry"
        );
    }
}

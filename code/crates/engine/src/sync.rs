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
use malachitebft_core_types::{CommitCertificate, Context};
use malachitebft_sync::{
    self as sync, HeightStartType, InboundRequestId, OutboundRequestId, RawDecidedValue, Request,
    Response, Resumable,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Timeout {
    Request(OutboundRequestId),
}

type Timers = TimerScheduler<Timeout>;

pub type SyncRef<Ctx> = ActorRef<Msg<Ctx>>;
pub type SyncMsg<Ctx> = Msg<Ctx>;

#[derive_where(Clone, Debug)]
pub struct RawDecidedBlock<Ctx: Context> {
    pub height: Ctx::Height,
    pub certificate: CommitCertificate<Ctx>,
    pub value_bytes: Bytes,
}

#[derive_where(Clone, Debug)]
pub struct InflightRequest<Ctx: Context> {
    pub peer_id: PeerId,
    pub request_id: OutboundRequestId,
    pub request: Request<Ctx>,
}

pub type InflightRequests<Ctx> = HashMap<OutboundRequestId, InflightRequest<Ctx>>;

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
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// We received an invalid value (either certificate or value) from a peer
    InvalidValue(PeerId, Ctx::Height),

    /// An error occurred while processing a value
    ValueProcessingError(PeerId, Ctx::Height),
}

impl<Ctx: Context> From<NetworkEvent<Ctx>> for Msg<Ctx> {
    fn from(event: NetworkEvent<Ctx>) -> Self {
        Msg::NetworkEvent(event)
    }
}

impl<Ctx: Context> From<TimeoutElapsed<Timeout>> for Msg<Ctx> {
    fn from(elapsed: TimeoutElapsed<Timeout>) -> Self {
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

/// The mode for sending status updates
enum StatusUpdateMode {
    /// Send status updates at regular intervals
    Interval(JoinHandle<()>), // the ticker task handle

    /// Send status updates with tip height when starting a new height
    OnStartedHeight,
}

pub struct State<Ctx: Context> {
    /// The state of the sync state machine
    sync: sync::State<Ctx>,

    /// Scheduler for timers
    timers: Timers,

    /// In-flight requests
    inflight: InflightRequests<Ctx>,

    /// Queue of sync value responses for heights ahead of consensus
    sync_queue: SyncQueue<Ctx>,

    /// Status update mode
    status_update_mode: StatusUpdateMode,
}

struct HandlerState<'a, Ctx: Context> {
    /// Scheduler for timers, used to start new timers for outgoing requests
    /// and correlate elapsed timers to the original request and peer.
    timers: &'a mut Timers,
    /// In-flight requests, used to correlate timeouts and responses to the original request and peer.
    inflight: &'a mut InflightRequests<Ctx>,
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

                if state.sync.peers.remove(&peer_id).is_some() {
                    debug!(%peer_id, "Removed disconnected peer");
                }
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

                // If in OnStartedHeight mode, send a status update for the previous decision,
                // now that we know for sure that the application has stored the decided value,
                // and we have updated our tip height.
                if let StatusUpdateMode::OnStartedHeight = &state.status_update_mode {
                    self.process_input(&myself, state, sync::Input::SendStatusUpdate)
                        .await?;
                }

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
            }

            // Received decided values from host
            //
            // We need to ensure that the total size of the response does not exceed the maximum allowed size.
            // If it does, we truncate the response accordingly.
            // This is to prevent sending overly large messages that could lead to network issues.
            Msg::GotDecidedValues(request_id, range, mut values) => {
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

            Msg::InvalidValue(peer, height) => {
                // Remove buffered values that came from the same request as the invalid value.
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

                self.process_input(&myself, state, sync::Input::InvalidValue(peer, height))
                    .await?
            }

            Msg::ValueProcessingError(peer, height) => {
                self.process_input(
                    &myself,
                    state,
                    sync::Input::ValueProcessingError(peer, height),
                )
                .await?
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
        info!("Using status update mode: OnStartedHeight");
        StatusUpdateMode::OnStartedHeight
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

        // NOTE: The queue capacity is set to accommodate all individual values for the
        // maximum number of parallel requests and batch size, with some additional buffer.
        let queue_capacity = 2 * self.sync_config.parallel_requests * self.sync_config.batch_size;

        Ok(State {
            sync: sync::State::new(rng, self.sync_config),
            timers: Timers::new(Box::new(myself.clone())),
            inflight: HashMap::new(),
            sync_queue: SyncQueue::new(queue_capacity),
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

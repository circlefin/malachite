use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use bytesize::ByteSize;
use derive_where::derive_where;
use eyre::eyre;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::{Rng as _, SeedableRng};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn, Instrument};

use malachitebft_codec as codec;
use malachitebft_core_consensus::PeerId;
use malachitebft_core_types::utils::height::DisplayRange;
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
    pub status_update_interval: Duration,
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

pub struct State<Ctx: Context> {
    /// The state of the sync state machine
    sync: sync::State<Ctx>,

    /// Scheduler for timers
    timers: Timers,

    /// In-flight requests
    inflight: InflightRequests<Ctx>,

    /// Task for sending status updates
    ticker: JoinHandle<()>,
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
        malachitebft_sync::process!(
            input: input,
            state: &mut state.sync,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(myself, &mut state.timers, &mut state.inflight, effect).await
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
        timers: &mut Timers,
        inflight: &mut InflightRequests<Ctx>,
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

                        timers.start_timer(
                            Timeout::Request(request_id.clone()),
                            self.params.request_timeout,
                        );

                        inflight.insert(
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

            Effect::ProcessValueResponse(peer_id, response, r) => {
                if let Err(e) = self
                    .consensus
                    .cast(ConsensusMsg::ProcessSyncResponse(peer_id, response))
                {
                    error!("Failed to forward value response to consensus: {e}");
                }

                Ok(r.resume_with(()))
            }
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
                self.process_input(&myself, state, sync::Input::Tick)
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
                self.process_input(&myself, state, sync::Input::StartedHeight(height, restart))
                    .await?
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

        // One-time uniform adjustment factor [-1%, +1%]
        const ADJ_RATE: f64 = 0.01;
        let adjustment = rng.gen_range(-ADJ_RATE..=ADJ_RATE);

        let ticker = tokio::spawn(
            ticker(
                self.params.status_update_interval,
                myself.clone(),
                adjustment,
                || Msg::Tick,
            )
            .in_current_span(),
        );

        Ok(State {
            sync: sync::State::new(rng, self.sync_config),
            timers: Timers::new(Box::new(myself.clone())),
            inflight: HashMap::new(),
            ticker,
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
        state.ticker.abort();
        Ok(())
    }
}

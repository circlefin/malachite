use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use derive_where::derive_where;
use eyre::eyre;

use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::SeedableRng;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn, Instrument};

use malachitebft_codec as codec;
use malachitebft_core_consensus::PeerId;
use malachitebft_core_types::{CertificateError, CommitCertificate, Context, Round};
use malachitebft_sync::{self as sync, InboundRequestId, OutboundRequestId, Response};
use malachitebft_sync::{RawDecidedValue, Request};

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
{
}

impl<Ctx, Codec> SyncCodec<Ctx> for Codec
where
    Ctx: Context,
    Codec: codec::Codec<sync::Status<Ctx>>,
    Codec: codec::Codec<sync::Request<Ctx>>,
    Codec: codec::Codec<sync::Response<Ctx>>,
{
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Timeout {
    Request(OutboundRequestId),
}

type Timers = TimerScheduler<Timeout>;

pub type SyncRef<Ctx> = ActorRef<Msg<Ctx>>;

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

#[derive_where(Debug)]
pub enum Msg<Ctx: Context> {
    /// Internal tick
    Tick,

    /// Receive an even from gossip layer
    NetworkEvent(NetworkEvent<Ctx>),

    /// Consensus has decided on a value at the given height
    Decided(Ctx::Height),

    /// Consensus has (re)started a new height.
    /// The boolean indicates whether this is a restart or not.
    StartedHeight(Ctx::Height, bool),

    /// Host has a response for the blocks request
    GotDecidedBlock(InboundRequestId, Ctx::Height, Option<RawDecidedValue<Ctx>>),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// We received an invalid [`CommitCertificate`] from a peer
    InvalidCertificate(PeerId, CommitCertificate<Ctx>, CertificateError<Ctx>),

    /// Consensus needs vote set from peers
    RequestVoteSet(Ctx::Height, Round),

    /// Consensus has sent a vote set response to a peer
    SentVoteSetResponse(InboundRequestId, Ctx::Height, Round),
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
pub struct Sync<Ctx: Context> {
    ctx: Ctx,
    gossip: NetworkRef<Ctx>,
    host: HostRef<Ctx>,
    params: Params,
    metrics: sync::Metrics,
    span: tracing::Span,
}

impl<Ctx> Sync<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        ctx: Ctx,
        gossip: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        params: Params,
        metrics: sync::Metrics,
        span: tracing::Span,
    ) -> Self {
        Self {
            ctx,
            gossip,
            host,
            params,
            metrics,
            span,
        }
    }

    pub async fn spawn(
        ctx: Ctx,
        gossip: NetworkRef<Ctx>,
        host: HostRef<Ctx>,
        params: Params,
        metrics: sync::Metrics,
        span: tracing::Span,
    ) -> Result<SyncRef<Ctx>, ractor::SpawnErr> {
        let actor = Self::new(ctx, gossip, host, params, metrics, span);
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
            Effect::BroadcastStatus(height) => {
                let history_min_height = self.get_history_min_height().await?;

                self.gossip.cast(NetworkMsg::BroadcastStatus(Status::new(
                    height,
                    history_min_height,
                )))?;
            }

            Effect::SendValueRequest(peer_id, value_request) => {
                let request = Request::ValueRequest(value_request);
                let result = ractor::call!(self.gossip, |reply_to| {
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
                                request_id,
                                request,
                            },
                        );
                    }
                    Err(e) => {
                        error!("Failed to send request to network layer: {e}");
                    }
                }
            }

            Effect::SendValueResponse(request_id, value_response) => {
                let response = Response::ValueResponse(value_response);
                self.gossip
                    .cast(NetworkMsg::OutgoingResponse(request_id, response))?;
            }

            Effect::GetDecidedValue(request_id, height) => {
                self.host.call_and_forward(
                    |reply_to| HostMsg::GetDecidedValue { height, reply_to },
                    myself,
                    move |synced_value| {
                        Msg::<Ctx>::GotDecidedBlock(request_id, height, synced_value)
                    },
                    None,
                )?;
            }
            Effect::SendVoteSetRequest(peer_id, vote_set_request) => {
                debug!(
                    height = %vote_set_request.height, round = %vote_set_request.round, peer = %peer_id,
                    "Send the vote set request to peer"
                );

                let request = Request::VoteSetRequest(vote_set_request);

                let result = ractor::call!(self.gossip, |reply_to| {
                    NetworkMsg::OutgoingRequest(peer_id, request.clone(), reply_to)
                });
                match result {
                    Ok(request_id) => {
                        timers.start_timer(
                            Timeout::Request(request_id.clone()),
                            self.params.request_timeout,
                        );

                        inflight.insert(
                            request_id.clone(),
                            InflightRequest {
                                peer_id,
                                request_id,
                                request,
                            },
                        );
                    }
                    Err(e) => {
                        error!("Failed to send request to gossip layer: {e}");
                    }
                }
            }
        }

        Ok(sync::Resume::default())
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::RequestVoteSet(height, round) => {
                debug!(%height, %round, "Make a vote set request to one of the peers");

                self.process_input(&myself, state, sync::Input::GetVoteSet(height, round))
                    .await?;
            }

            Msg::SentVoteSetResponse(request_id, height, round) => {
                self.process_input(
                    &myself,
                    state,
                    sync::Input::GotVoteSet(request_id, height, round),
                )
                .await?;
            }

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

            Msg::NetworkEvent(NetworkEvent::Request(request_id, from, request)) => {
                match request {
                    Request::ValueRequest(value_request) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::ValueRequest(request_id, from, value_request),
                        )
                        .await?;
                    }
                    Request::VoteSetRequest(vote_set_request) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::VoteSetRequest(request_id, from, vote_set_request),
                        )
                        .await?;
                    }
                };
            }

            Msg::NetworkEvent(NetworkEvent::Response(request_id, peer, response)) => {
                // Cancel the timer associated with the request for which we just received a response
                state.timers.cancel(&Timeout::Request(request_id.clone()));

                match response {
                    Response::ValueResponse(value_response) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::ValueResponse(request_id, peer, value_response),
                        )
                        .await?;
                    }
                    Response::VoteSetResponse(vote_set_response) => {
                        self.process_input(
                            &myself,
                            state,
                            sync::Input::VoteSetResponse(request_id, peer, vote_set_response),
                        )
                        .await?;
                    }
                }
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

            Msg::GotDecidedBlock(request_id, height, block) => {
                self.process_input(
                    &myself,
                    state,
                    sync::Input::GotDecidedValue(request_id, height, block),
                )
                .await?;
            }

            Msg::InvalidCertificate(peer, certificate, error) => {
                self.process_input(
                    &myself,
                    state,
                    sync::Input::InvalidCertificate(peer, certificate, error),
                )
                .await?
            }

            Msg::TimeoutElapsed(elapsed) => {
                let Some(timeout) = state.timers.intercept_timer_msg(elapsed) else {
                    // Timer was cancelled or already processed, ignore
                    return Ok(());
                };

                warn!(?timeout, "Timeout elapsed");

                match timeout {
                    Timeout::Request(request_id) => {
                        if let Some(inflight) = state.inflight.remove(&request_id) {
                            self.process_input(
                                &myself,
                                state,
                                sync::Input::SyncRequestTimedOut(
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

#[async_trait]
impl<Ctx> Actor for Sync<Ctx>
where
    Ctx: Context,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        self.gossip
            .cast(NetworkMsg::Subscribe(Box::new(myself.clone())))?;

        let ticker = tokio::spawn(
            ticker(self.params.status_update_interval, myself.clone(), || {
                Msg::Tick
            })
            .in_current_span(),
        );

        let rng = Box::new(rand::rngs::StdRng::from_entropy());

        Ok(State {
            sync: sync::State::new(rng),
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
            height.tip = %state.sync.tip_height,
            height.sync = %state.sync.sync_height,
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

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use derive_where::derive_where;
use eyre::eyre;

use ractor::{Actor, ActorProcessingErr, ActorRef};
use rand::SeedableRng;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use malachite_blocksync::{self as blocksync, InboundRequestId, OutboundRequestId, Response};
use malachite_blocksync::{Request, SyncedBlock};
use malachite_codec as codec;
use malachite_common::{CertificateError, CommitCertificate, Context, Height, Round};
use malachite_consensus::PeerId;

use crate::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef, GossipEvent, Status};
use crate::host::{HostMsg, HostRef};
use crate::util::forward::forward;
use crate::util::ticker::ticker;
use crate::util::timers::{TimeoutElapsed, TimerScheduler};

/// Codec for sync protocol messages
///
/// This trait is automatically implemented for any type that implements:
/// - [`codec::Codec<blocksync::Status<Ctx>>`]
/// - [`codec::Codec<blocksync::Request<Ctx>>`]
/// - [`codec::Codec<blocksync::Response<Ctx>>`]
pub trait BlockSyncCodec<Ctx>
where
    Ctx: Context,
    Self: codec::Codec<blocksync::Status<Ctx>>,
    Self: codec::Codec<blocksync::Request<Ctx>>,
    Self: codec::Codec<blocksync::Response<Ctx>>,
{
}

impl<Ctx, Codec> BlockSyncCodec<Ctx> for Codec
where
    Ctx: Context,
    Codec: codec::Codec<blocksync::Status<Ctx>>,
    Codec: codec::Codec<blocksync::Request<Ctx>>,
    Codec: codec::Codec<blocksync::Response<Ctx>>,
{
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Timeout {
    Request(OutboundRequestId),
}

type Timers<Ctx> = TimerScheduler<Timeout, Msg<Ctx>>;

pub type BlockSyncRef<Ctx> = ActorRef<Msg<Ctx>>;

#[derive_where(Clone, Debug)]
pub struct RawDecidedBlock<Ctx: Context> {
    pub height: Ctx::Height,
    pub certificate: CommitCertificate<Ctx>,
    pub block_bytes: Bytes,
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
    GossipEvent(GossipEvent<Ctx>),

    /// Consensus has decided on a value at the given height
    Decided(Ctx::Height),

    /// Consensus has started a new height
    StartedHeight(Ctx::Height),

    /// Host has a response for the blocks request
    GotDecidedBlock(InboundRequestId, Ctx::Height, Option<SyncedBlock<Ctx>>),

    /// A timeout has elapsed
    TimeoutElapsed(TimeoutElapsed<Timeout>),

    /// We received an invalid [`CommitCertificate`] from a peer
    InvalidCertificate(PeerId, CommitCertificate<Ctx>, CertificateError<Ctx>),

    /// Consensus needs vote set from peers
    RequestVoteSet(Ctx::Height, Round),

    /// Consensus has sent a vote set response to a peer
    SentVoteSetResponse(InboundRequestId, Ctx::Height, Round),
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

pub struct Args<Ctx: Context> {
    pub initial_height: Ctx::Height,
}

pub struct State<Ctx: Context> {
    /// The state of the blocksync state machine
    blocksync: blocksync::State<Ctx>,

    /// Scheduler for timers
    timers: Timers<Ctx>,

    /// In-flight requests
    inflight: InflightRequests<Ctx>,

    /// Task for sending status updates
    ticker: JoinHandle<()>,
}

#[allow(dead_code)]
pub struct BlockSync<Ctx: Context> {
    ctx: Ctx,
    gossip: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    params: Params,
    metrics: blocksync::Metrics,
    span: tracing::Span,
}

impl<Ctx> BlockSync<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        ctx: Ctx,
        gossip: GossipConsensusRef<Ctx>,
        host: HostRef<Ctx>,
        params: Params,
        metrics: blocksync::Metrics,
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
        self,
        initial_height: Ctx::Height,
    ) -> Result<BlockSyncRef<Ctx>, ractor::SpawnErr> {
        let (actor_ref, _) = Actor::spawn(None, self, Args { initial_height }).await?;
        Ok(actor_ref)
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: blocksync::Input<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        malachite_blocksync::process!(
            input: input,
            state: &mut state.blocksync,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(myself, &mut state.timers, &mut state.inflight, effect).await
            }
        )
    }

    async fn get_earliest_block_height(&self) -> Result<Ctx::Height, ActorProcessingErr> {
        ractor::call!(self.host, |reply_to| HostMsg::GetEarliestBlockHeight {
            reply_to
        })
        .map_err(|e| eyre!("Failed to get earliest block height: {e:?}").into())
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        timers: &mut Timers<Ctx>,
        inflight: &mut InflightRequests<Ctx>,
        effect: blocksync::Effect<Ctx>,
    ) -> Result<blocksync::Resume<Ctx>, ActorProcessingErr> {
        use blocksync::Effect;

        match effect {
            Effect::BroadcastStatus(height) => {
                let earliest_block_height = self.get_earliest_block_height().await?;

                self.gossip
                    .cast(GossipConsensusMsg::BroadcastStatus(Status::new(
                        height,
                        earliest_block_height,
                    )))?;
            }

            Effect::SendBlockRequest(peer_id, block_request) => {
                let request = Request::BlockRequest(block_request);
                let result = ractor::call!(self.gossip, |reply_to| {
                    GossipConsensusMsg::OutgoingRequest(peer_id, request.clone(), reply_to)
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
                        error!("Failed to send request to gossip layer: {e}");
                    }
                }
            }

            Effect::SendBlockResponse(request_id, block_response) => {
                let response = Response::BlockResponse(block_response);
                self.gossip
                    .cast(GossipConsensusMsg::OutgoingResponse(request_id, response))?;
            }

            Effect::GetBlock(request_id, height) => {
                self.host.call_and_forward(
                    |reply_to| HostMsg::GetDecidedBlock { height, reply_to },
                    myself,
                    move |block| Msg::<Ctx>::GotDecidedBlock(request_id, height, block),
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
                    GossipConsensusMsg::OutgoingRequest(peer_id, request.clone(), reply_to)
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

        Ok(blocksync::Resume::default())
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

                self.process_input(&myself, state, blocksync::Input::GetVoteSet(height, round))
                    .await?;
            }

            Msg::SentVoteSetResponse(request_id, height, round) => {
                self.process_input(
                    &myself,
                    state,
                    blocksync::Input::GotVoteSet(request_id, height, round),
                )
                .await?;
            }

            Msg::Tick => {
                self.process_input(&myself, state, blocksync::Input::Tick)
                    .await?;
            }

            Msg::GossipEvent(GossipEvent::PeerDisconnected(peer_id)) => {
                info!(%peer_id, "Disconnected from peer");

                if state.blocksync.peers.remove(&peer_id).is_some() {
                    debug!(%peer_id, "Removed disconnected peer");
                }
            }

            Msg::GossipEvent(GossipEvent::Status(peer_id, status)) => {
                let status = blocksync::Status {
                    peer_id,
                    height: status.height,
                    earliest_block_height: status.earliest_block_height,
                };

                self.process_input(&myself, state, blocksync::Input::Status(status))
                    .await?;
            }

            Msg::GossipEvent(GossipEvent::Request(request_id, from, request)) => {
                match request {
                    Request::BlockRequest(block_request) => {
                        self.process_input(
                            &myself,
                            state,
                            blocksync::Input::BlockRequest(request_id, from, block_request),
                        )
                        .await?;
                    }
                    Request::VoteSetRequest(vote_set_request) => {
                        self.process_input(
                            &myself,
                            state,
                            blocksync::Input::VoteSetRequest(request_id, from, vote_set_request),
                        )
                        .await?;
                    }
                };
            }

            Msg::GossipEvent(GossipEvent::Response(request_id, peer, response)) => {
                // Cancel the timer associated with the request for which we just received a response
                state.timers.cancel(&Timeout::Request(request_id.clone()));

                match response {
                    Response::BlockResponse(block_response) => {
                        self.process_input(
                            &myself,
                            state,
                            blocksync::Input::BlockResponse(request_id, peer, block_response),
                        )
                        .await?;
                    }
                    Response::VoteSetResponse(vote_set_response) => {
                        self.process_input(
                            &myself,
                            state,
                            blocksync::Input::VoteSetResponse(request_id, peer, vote_set_response),
                        )
                        .await?;
                    }
                }
            }

            Msg::GossipEvent(_) => {
                // Ignore other gossip events
            }

            Msg::Decided(height) => {
                self.process_input(&myself, state, blocksync::Input::UpdateHeight(height))
                    .await?;
            }

            Msg::StartedHeight(height) => {
                if let Some(height) = height.decrement() {
                    self.process_input(&myself, state, blocksync::Input::UpdateHeight(height))
                        .await?;
                }

                self.process_input(&myself, state, blocksync::Input::StartHeight(height))
                    .await?;
            }

            Msg::GotDecidedBlock(request_id, height, block) => {
                self.process_input(
                    &myself,
                    state,
                    blocksync::Input::GotBlock(request_id, height, block),
                )
                .await?;
            }

            Msg::InvalidCertificate(peer, certificate, error) => {
                self.process_input(
                    &myself,
                    state,
                    blocksync::Input::InvalidCertificate(peer, certificate, error),
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
                                blocksync::Input::SyncRequestTimedOut(
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
impl<Ctx> Actor for BlockSync<Ctx>
where
    Ctx: Context,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args<Ctx>;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Args<Ctx>,
    ) -> Result<Self::State, ActorProcessingErr> {
        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;
        self.gossip.cast(GossipConsensusMsg::Subscribe(forward))?;

        let ticker = tokio::spawn(ticker(
            self.params.status_update_interval,
            myself.clone(),
            || Msg::Tick,
        ));

        let rng = Box::new(rand::rngs::StdRng::from_entropy());

        Ok(State {
            blocksync: blocksync::State::new(rng, args.initial_height),
            timers: Timers::new(myself.clone()),
            inflight: HashMap::new(),
            ticker,
        })
    }

    #[tracing::instrument(name = "blocksync", parent = &self.span, skip_all)]
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

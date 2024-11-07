use std::collections::BTreeSet;
use std::marker::PhantomData;

use async_trait::async_trait;
use derive_where::derive_where;
use libp2p::identity::Keypair;
use libp2p::request_response::{InboundRequestId, OutboundRequestId};
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::task::JoinHandle;
use tracing::{error, trace};

use malachite_blocksync::{self as blocksync, Response};
use malachite_blocksync::{RawMessage, Request};
use malachite_common::{Context, SignedProposal, SignedVote};
use malachite_consensus::SignedConsensusMsg;
use malachite_gossip_consensus::handle::CtrlHandle;
use malachite_gossip_consensus::{Channel, Config, Event, Multiaddr, PeerId};
use malachite_metrics::SharedRegistry;

use crate::util::codec::NetworkCodec;
use crate::util::streaming::StreamMessage;

pub type GossipConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;
pub type GossipConsensusMsg<Ctx> = Msg<Ctx>;

pub struct GossipConsensus<Ctx, Codec> {
    codec: Codec,
    marker: PhantomData<Ctx>,
}

impl<Ctx, Codec> GossipConsensus<Ctx, Codec> {
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            marker: PhantomData,
        }
    }
}

impl<Ctx, Codec> GossipConsensus<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<Ctx::ProposalPart>,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
    Codec: NetworkCodec<StreamMessage<Ctx::ProposalPart>>,
    Codec: NetworkCodec<blocksync::Status<Ctx>>,
    Codec: NetworkCodec<blocksync::Request<Ctx>>,
    Codec: NetworkCodec<blocksync::Response<Ctx>>,
{
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
        codec: Codec,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let args = Args {
            keypair,
            config,
            metrics,
        };

        let (actor_ref, _) = Actor::spawn(None, Self::new(codec), args).await?;
        Ok(actor_ref)
    }

    fn publish(&self, event: GossipEvent<Ctx>, subscribers: &mut [ActorRef<GossipEvent<Ctx>>]) {
        if let Some((last, head)) = subscribers.split_last() {
            for subscriber in head {
                let _ = subscriber.cast(event.clone());
            }

            let _ = last.cast(event);
        }
    }
}

pub struct Args {
    pub keypair: Keypair,
    pub config: Config,
    pub metrics: SharedRegistry,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum GossipEvent<Ctx: Context> {
    Listening(Multiaddr),

    PeerConnected(PeerId),
    PeerDisconnected(PeerId),

    Vote(PeerId, SignedVote<Ctx>),

    Proposal(PeerId, SignedProposal<Ctx>),
    ProposalPart(PeerId, StreamMessage<Ctx::ProposalPart>),

    Status(PeerId, Status<Ctx>),

    BlockSyncRequest(InboundRequestId, PeerId, Request<Ctx>),
    BlockSyncResponse(OutboundRequestId, Response<Ctx>),
}

pub enum State<Ctx: Context> {
    Stopped,
    Running {
        peers: BTreeSet<PeerId>,
        subscribers: Vec<ActorRef<GossipEvent<Ctx>>>,
        ctrl_handle: CtrlHandle,
        recv_task: JoinHandle<()>,
        marker: PhantomData<Ctx>,
    },
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub height: Ctx::Height,
    pub earliest_block_height: Ctx::Height,
}

impl<Ctx: Context> Status<Ctx> {
    pub fn new(height: Ctx::Height, earliest_block_height: Ctx::Height) -> Self {
        Self {
            height,
            earliest_block_height,
        }
    }
}

pub enum Msg<Ctx: Context> {
    /// Subscribe this actor to receive gossip events
    Subscribe(ActorRef<GossipEvent<Ctx>>),

    /// Publish a signed consensus message
    Publish(SignedConsensusMsg<Ctx>),

    /// Publish a proposal part
    PublishProposalPart(StreamMessage<Ctx::ProposalPart>),

    /// Publish status
    PublishStatus(Status<Ctx>),

    /// Send a request to a peer, returning the outbound request ID
    OutgoingBlockSyncRequest(PeerId, Request<Ctx>, RpcReplyPort<OutboundRequestId>),

    /// Send a response for a blocks request to a peer
    OutgoingBlockSyncResponse(InboundRequestId, Response<Ctx>),

    /// Request for number of peers from gossip
    GetState { reply: RpcReplyPort<usize> },

    // Event emitted by the gossip layer
    #[doc(hidden)]
    NewEvent(Event),
}

#[async_trait]
impl<Ctx, Codec> Actor for GossipConsensus<Ctx, Codec>
where
    Ctx: Context,
    Codec: Send + Sync + 'static,
    Codec: NetworkCodec<Ctx::ProposalPart>,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
    Codec: NetworkCodec<StreamMessage<Ctx::ProposalPart>>,
    Codec: NetworkCodec<blocksync::Status<Ctx>>,
    Codec: NetworkCodec<blocksync::Request<Ctx>>,
    Codec: NetworkCodec<blocksync::Response<Ctx>>,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        args: Args,
    ) -> Result<Self::State, ActorProcessingErr> {
        let handle =
            malachite_gossip_consensus::spawn(args.keypair, args.config, args.metrics).await?;

        let (mut recv_handle, ctrl_handle) = handle.split();

        let recv_task = tokio::spawn(async move {
            while let Some(event) = recv_handle.recv().await {
                if let Err(e) = myself.cast(Msg::NewEvent(event)) {
                    error!("Actor has died, stopping gossip consensus: {e:?}");
                    break;
                }
            }
        });

        Ok(State::Running {
            peers: BTreeSet::new(),
            subscribers: Vec::new(),
            ctrl_handle,
            recv_task,
            marker: PhantomData,
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        _state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let State::Running {
            peers,
            subscribers,
            ctrl_handle,
            ..
        } = state
        else {
            return Ok(());
        };

        match msg {
            Msg::Subscribe(subscriber) => subscribers.push(subscriber),

            Msg::Publish(msg) => match self.codec.encode(msg) {
                Ok(data) => ctrl_handle.publish(Channel::Consensus, data).await?,
                Err(e) => error!("Failed to encode gossip message: {e:?}"),
            },

            Msg::PublishProposalPart(msg) => {
                trace!(
                    stream_id = %msg.stream_id,
                    sequence = %msg.sequence,
                    "Broadcasting proposal part"
                );

                let data = self.codec.encode(msg);
                match data {
                    Ok(data) => ctrl_handle.publish(Channel::ProposalParts, data).await?,
                    Err(e) => error!("Failed to encode proposal part: {e:?}"),
                }
            }

            Msg::PublishStatus(status) => {
                let status = blocksync::Status {
                    peer_id: ctrl_handle.peer_id(),
                    height: status.height,
                    earliest_block_height: status.earliest_block_height,
                };

                let data = self.codec.encode(status);
                match data {
                    Ok(data) => ctrl_handle.publish(Channel::BlockSync, data).await?,
                    Err(e) => error!("Failed to encode status message: {e:?}"),
                }
            }

            Msg::OutgoingBlockSyncRequest(peer_id, request, reply_to) => {
                let request = self.codec.encode(request);
                match request {
                    Ok(data) => {
                        let request_id = ctrl_handle.blocksync_request(peer_id, data).await?;
                        reply_to.send(request_id)?;
                    }
                    Err(e) => error!("Failed to encode request message: {e:?}"),
                }
            }

            Msg::OutgoingBlockSyncResponse(request_id, response) => {
                let msg = match self.codec.encode(response) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(%request_id, "Failed to encode block response message: {e:?}");
                        return Ok(());
                    }
                };

                ctrl_handle.blocksync_reply(request_id, msg).await?
            }

            Msg::NewEvent(Event::Listening(addr)) => {
                self.publish(GossipEvent::Listening(addr), subscribers);
            }

            Msg::NewEvent(Event::PeerConnected(peer_id)) => {
                peers.insert(peer_id);
                self.publish(GossipEvent::PeerConnected(peer_id), subscribers);
            }

            Msg::NewEvent(Event::PeerDisconnected(peer_id)) => {
                peers.remove(&peer_id);
                self.publish(GossipEvent::PeerDisconnected(peer_id), subscribers);
            }

            Msg::NewEvent(Event::Message(Channel::Consensus, from, data)) => {
                let msg = match self.codec.decode(data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(%from, "Failed to decode gossip message: {e:?}");
                        return Ok(());
                    }
                };

                let event = match msg {
                    SignedConsensusMsg::Vote(vote) => GossipEvent::Vote(from, vote),
                    SignedConsensusMsg::Proposal(proposal) => GossipEvent::Proposal(from, proposal),
                };

                self.publish(event, subscribers);
            }

            Msg::NewEvent(Event::Message(Channel::ProposalParts, from, data)) => {
                let msg: StreamMessage<Ctx::ProposalPart> = match self.codec.decode(data) {
                    Ok(stream_msg) => stream_msg,
                    Err(e) => {
                        error!(%from, "Failed to decode stream message: {e:?}");
                        return Ok(());
                    }
                };

                trace!(
                    %from,
                    stream_id = %msg.stream_id,
                    sequence = %msg.sequence,
                    "Received proposal part"
                );

                self.publish(GossipEvent::ProposalPart(from, msg), subscribers);
            }

            Msg::NewEvent(Event::Message(Channel::BlockSync, from, data)) => {
                let status: blocksync::Status<Ctx> = match self.codec.decode(data) {
                    Ok(status) => status,
                    Err(e) => {
                        error!(%from, "Failed to decode status message: {e:?}");
                        return Ok(());
                    }
                };

                if from != status.peer_id {
                    error!(%from, %status.peer_id, "Mismatched peer ID in status message");
                    return Ok(());
                }

                trace!(%from, height = %status.height, "Received status");

                self.publish(
                    GossipEvent::Status(
                        status.peer_id,
                        Status::new(status.height, status.earliest_block_height),
                    ),
                    subscribers,
                );
            }

            Msg::NewEvent(Event::BlockSync(raw_msg)) => match raw_msg {
                RawMessage::Request {
                    request_id,
                    peer,
                    body,
                } => {
                    let request: blocksync::Request<Ctx> = match self.codec.decode(body) {
                        Ok(request) => request,
                        Err(e) => {
                            error!(%peer, "Failed to decode BlockSync request: {e:?}");
                            return Ok(());
                        }
                    };

                    self.publish(
                        GossipEvent::BlockSyncRequest(request_id, peer, request),
                        subscribers,
                    );
                }

                RawMessage::Response { request_id, body } => {
                    let response: blocksync::Response<Ctx> = match self.codec.decode(body) {
                        Ok(response) => response,
                        Err(e) => {
                            error!("Failed to decode BlockSync response: {e:?}");
                            return Ok(());
                        }
                    };

                    self.publish(
                        GossipEvent::BlockSyncResponse(request_id, response),
                        subscribers,
                    );
                }
            },

            Msg::GetState { reply } => {
                let number_peers = match state {
                    State::Stopped => 0,
                    State::Running { peers, .. } => peers.len(),
                };
                reply.send(number_peers)?;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let state = std::mem::replace(state, State::Stopped);

        if let State::Running {
            ctrl_handle,
            recv_task,
            ..
        } = state
        {
            ctrl_handle.wait_shutdown().await?;
            recv_task.await?;
        }

        Ok(())
    }
}

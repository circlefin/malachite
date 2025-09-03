use std::collections::{BTreeSet, HashMap};
use std::marker::PhantomData;

use async_trait::async_trait;
use derive_where::derive_where;
use eyre::eyre;
use libp2p::identity::Keypair;
use libp2p::request_response;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::task::JoinHandle;
use tracing::{error, trace};

use malachitebft_sync::{
    self as sync, InboundRequestId, OutboundRequestId, RawMessage, Request, Response,
};

use malachitebft_codec as codec;
use malachitebft_core_consensus::{LivenessMsg, SignedConsensusMsg};
use malachitebft_core_types::{
    Context, PolkaCertificate, RoundCertificate, SignedProposal, SignedVote,
};
use malachitebft_metrics::SharedRegistry;
use malachitebft_network::handle::CtrlHandle;
use malachitebft_network::{Channel, Config, Event, Multiaddr, PeerId};

use crate::consensus::ConsensusCodec;
use crate::sync::SyncCodec;
use crate::util::output_port::{OutputPort, OutputPortSubscriberTrait};
use crate::util::streaming::StreamMessage;

pub type NetworkRef<Ctx> = ActorRef<Msg<Ctx>>;
pub type NetworkMsg<Ctx> = Msg<Ctx>;

pub trait Subscriber<Msg>: OutputPortSubscriberTrait<Msg>
where
    Msg: Clone + ractor::Message,
{
    fn send(&self, msg: Msg);
}

impl<Msg, To> Subscriber<Msg> for ActorRef<To>
where
    Msg: Clone + ractor::Message,
    To: From<Msg> + ractor::Message,
{
    fn send(&self, msg: Msg) {
        if let Err(e) = self.cast(To::from(msg)) {
            error!("Failed to send message to subscriber: {e:?}");
        }
    }
}

pub struct Network<Ctx, Codec> {
    codec: Codec,
    span: tracing::Span,
    marker: PhantomData<Ctx>,
}

impl<Ctx, Codec> Network<Ctx, Codec> {
    pub fn new(codec: Codec, span: tracing::Span) -> Self {
        Self {
            codec,
            span,
            marker: PhantomData,
        }
    }
}

impl<Ctx, Codec> Network<Ctx, Codec>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
        codec: Codec,
        span: tracing::Span,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let args = Args {
            keypair,
            config: config.clone(),
            metrics,
        };

        let (actor_ref, _) = Actor::spawn(None, Self::new(codec, span), args).await?;
        Ok(actor_ref)
    }
}

pub struct Args {
    pub keypair: Keypair,
    pub config: Config,
    pub metrics: SharedRegistry,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum NetworkEvent<Ctx: Context> {
    Listening(Multiaddr),

    PeerConnected(PeerId),
    PeerDisconnected(PeerId),

    Vote(PeerId, SignedVote<Ctx>),

    Proposal(PeerId, SignedProposal<Ctx>),
    ProposalPart(PeerId, StreamMessage<Ctx::ProposalPart>),

    PolkaCertificate(PeerId, PolkaCertificate<Ctx>),

    RoundCertificate(PeerId, RoundCertificate<Ctx>),

    Status(PeerId, Status<Ctx>),

    SyncRequest(InboundRequestId, PeerId, Request<Ctx>),
    SyncResponse(OutboundRequestId, PeerId, Option<Response<Ctx>>),
}

pub enum State<Ctx: Context> {
    Stopped,
    Running {
        listen_addrs: Vec<Multiaddr>,
        peers: BTreeSet<PeerId>,
        output_port: OutputPort<NetworkEvent<Ctx>>,
        ctrl_handle: Box<CtrlHandle>,
        recv_task: JoinHandle<()>,
        inbound_requests: HashMap<InboundRequestId, request_response::InboundRequestId>,
    },
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub tip_height: Ctx::Height,
    pub history_min_height: Ctx::Height,
}

impl<Ctx: Context> Status<Ctx> {
    pub fn new(tip_height: Ctx::Height, history_min_height: Ctx::Height) -> Self {
        Self {
            tip_height,
            history_min_height,
        }
    }
}

pub enum Msg<Ctx: Context> {
    /// Subscribe this actor to receive gossip events
    Subscribe(Box<dyn Subscriber<NetworkEvent<Ctx>>>),

    /// Publish a signed consensus message
    PublishConsensusMsg(SignedConsensusMsg<Ctx>),

    /// Publish a liveness message
    PublishLivenessMsg(LivenessMsg<Ctx>),

    /// Publish a proposal part
    PublishProposalPart(StreamMessage<Ctx::ProposalPart>),

    /// Broadcast status to all direct peers
    BroadcastStatus(Status<Ctx>),

    /// Send a request to a peer, returning the outbound request ID
    OutgoingRequest(PeerId, Request<Ctx>, RpcReplyPort<OutboundRequestId>),

    /// Send a response for a request to a peer
    OutgoingResponse(InboundRequestId, Response<Ctx>),

    /// Request the total number of bytes that are to be transmitted for this response
    GetResponseSize(Response<Ctx>, RpcReplyPort<usize>),

    /// Request for number of peers from gossip
    GetState { reply: RpcReplyPort<usize> },

    // Event emitted by the gossip layer
    #[doc(hidden)]
    NewEvent(Event),
}

#[async_trait]
impl<Ctx, Codec> Actor for Network<Ctx, Codec>
where
    Ctx: Context,
    Codec: Send + Sync + 'static,
    Codec: codec::Codec<Ctx::ProposalPart>,
    Codec: codec::Codec<SignedConsensusMsg<Ctx>>,
    Codec: codec::Codec<StreamMessage<Ctx::ProposalPart>>,
    Codec: codec::Codec<sync::Status<Ctx>>,
    Codec: codec::Codec<sync::Request<Ctx>>,
    Codec: codec::Codec<sync::Response<Ctx>>,
    Codec: codec::Codec<LivenessMsg<Ctx>>,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        args: Args,
    ) -> Result<Self::State, ActorProcessingErr> {
        let handle = malachitebft_network::spawn(args.keypair, args.config, args.metrics).await?;

        let (mut recv_handle, ctrl_handle) = handle.split();

        let recv_task = tokio::spawn(async move {
            while let Some(event) = recv_handle.recv().await {
                if let Err(e) = myself.cast(Msg::NewEvent(event)) {
                    error!("Actor has died, stopping network: {e:?}");
                    break;
                }
            }
        });

        Ok(State::Running {
            listen_addrs: Vec::new(),
            peers: BTreeSet::new(),
            output_port: OutputPort::with_capacity(128),
            ctrl_handle: Box::new(ctrl_handle),
            recv_task,
            inbound_requests: HashMap::new(),
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        _state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    #[tracing::instrument(name = "network", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let State::Running {
            listen_addrs,
            peers,
            output_port,
            ctrl_handle,
            inbound_requests,
            ..
        } = state
        else {
            return Ok(());
        };

        match msg {
            Msg::Subscribe(subscriber) => {
                for addr in listen_addrs.iter() {
                    subscriber.send(NetworkEvent::Listening(addr.clone()));
                }

                for peer in peers.iter() {
                    subscriber.send(NetworkEvent::PeerConnected(*peer));
                }

                subscriber.subscribe_to_port(output_port);
            }

            Msg::PublishConsensusMsg(msg) => match self.codec.encode(&msg) {
                Ok(data) => ctrl_handle.publish(Channel::Consensus, data).await?,
                Err(e) => error!("Failed to encode consensus message: {e:?}"),
            },

            Msg::PublishLivenessMsg(msg) => match self.codec.encode(&msg) {
                Ok(data) => ctrl_handle.publish(Channel::Liveness, data).await?,
                Err(e) => error!("Failed to encode liveness message: {e:?}"),
            },

            Msg::PublishProposalPart(msg) => {
                trace!(
                    stream_id = %msg.stream_id,
                    sequence = %msg.sequence,
                    "Broadcasting proposal part"
                );

                let data = self.codec.encode(&msg);
                match data {
                    Ok(data) => ctrl_handle.publish(Channel::ProposalParts, data).await?,
                    Err(e) => error!("Failed to encode proposal part: {e:?}"),
                }
            }

            Msg::BroadcastStatus(status) => {
                let status = sync::Status {
                    peer_id: ctrl_handle.peer_id(),
                    tip_height: status.tip_height,
                    history_min_height: status.history_min_height,
                };

                let data = self.codec.encode(&status);
                match data {
                    Ok(data) => ctrl_handle.broadcast(Channel::Sync, data).await?,
                    Err(e) => error!("Failed to encode status message: {e:?}"),
                }
            }

            Msg::OutgoingRequest(peer_id, request, reply_to) => {
                let request = self.codec.encode(&request);

                match request {
                    Ok(data) => {
                        let p2p_request_id = ctrl_handle.sync_request(peer_id, data).await?;
                        reply_to.send(OutboundRequestId::new(p2p_request_id))?;
                    }
                    Err(e) => error!("Failed to encode request message: {e:?}"),
                }
            }

            Msg::OutgoingResponse(request_id, response) => {
                let response = self.codec.encode(&response);

                match response {
                    Ok(data) => {
                        let request_id = inbound_requests
                            .remove(&request_id)
                            .ok_or_else(|| eyre!("Unknown inbound request ID: {request_id}"))?;

                        ctrl_handle.sync_reply(request_id, data).await?
                    }
                    Err(e) => {
                        error!(%request_id, "Failed to encode response message: {e:?}");
                        return Ok(());
                    }
                };
            }

            Msg::GetResponseSize(response, reply_to) => {
                let encoded = self.codec.encode(&response);

                match encoded {
                    Ok(data) => {
                        reply_to.send(data.len())?;
                    }
                    Err(e) => {
                        error!(?response, "Failed to encode response message: {e:?}");
                        return Ok(());
                    }
                };
            }

            Msg::NewEvent(Event::Listening(addr)) => {
                listen_addrs.push(addr.clone());
                output_port.send(NetworkEvent::Listening(addr));
            }

            Msg::NewEvent(Event::PeerConnected(peer_id)) => {
                peers.insert(peer_id);
                output_port.send(NetworkEvent::PeerConnected(peer_id));
            }

            Msg::NewEvent(Event::PeerDisconnected(peer_id)) => {
                peers.remove(&peer_id);
                output_port.send(NetworkEvent::PeerDisconnected(peer_id));
            }

            Msg::NewEvent(Event::LivenessMessage(Channel::Liveness, from, data)) => {
                let msg = match self.codec.decode(data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(%from, "Failed to decode liveness message: {e:?}");
                        return Ok(());
                    }
                };

                let event = match msg {
                    LivenessMsg::PolkaCertificate(polka_cert) => {
                        NetworkEvent::PolkaCertificate(from, polka_cert)
                    }
                    LivenessMsg::SkipRoundCertificate(round_cert) => {
                        NetworkEvent::RoundCertificate(from, round_cert)
                    }
                    LivenessMsg::Vote(vote) => NetworkEvent::Vote(from, vote),
                };

                output_port.send(event);
            }

            Msg::NewEvent(Event::LivenessMessage(channel, from, _)) => {
                error!(%from, "Unexpected liveness message on {channel} channel");
                return Ok(());
            }

            Msg::NewEvent(Event::ConsensusMessage(Channel::Consensus, from, data)) => {
                let msg = match self.codec.decode(data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(%from, "Failed to decode consensus message: {e:?}");
                        return Ok(());
                    }
                };

                let event = match msg {
                    SignedConsensusMsg::Vote(vote) => NetworkEvent::Vote(from, vote),
                    SignedConsensusMsg::Proposal(proposal) => {
                        NetworkEvent::Proposal(from, proposal)
                    }
                };

                output_port.send(event);
            }

            Msg::NewEvent(Event::ConsensusMessage(Channel::ProposalParts, from, data)) => {
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

                output_port.send(NetworkEvent::ProposalPart(from, msg));
            }

            Msg::NewEvent(Event::ConsensusMessage(Channel::Sync, from, data)) => {
                let status: sync::Status<Ctx> = match self.codec.decode(data) {
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

                trace!(%from, tip_height = %status.tip_height, "Received status");

                output_port.send(NetworkEvent::Status(
                    status.peer_id,
                    Status::new(status.tip_height, status.history_min_height),
                ));
            }

            Msg::NewEvent(Event::ConsensusMessage(channel, from, _)) => {
                error!(%from, "Unexpected consensus message on {channel} channel");
                return Ok(());
            }

            Msg::NewEvent(Event::Sync(raw_msg)) => match raw_msg {
                RawMessage::Request {
                    request_id,
                    peer,
                    body,
                } => {
                    let request = match self.codec.decode(body) {
                        Ok(request) => request,
                        Err(e) => {
                            error!(%peer, "Failed to decode sync request: {e:?}");
                            return Ok(());
                        }
                    };

                    inbound_requests.insert(InboundRequestId::new(request_id), request_id);

                    output_port.send(NetworkEvent::SyncRequest(
                        InboundRequestId::new(request_id),
                        peer,
                        request,
                    ));
                }

                RawMessage::Response {
                    request_id,
                    peer,
                    body,
                } => {
                    let response = match self.codec.decode(body) {
                        Ok(response) => Some(response),
                        Err(e) => {
                            error!(%peer, "Failed to decode sync response: {e:?}");
                            None
                        }
                    };

                    output_port.send(NetworkEvent::SyncResponse(
                        OutboundRequestId::new(request_id),
                        peer,
                        response,
                    ));
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

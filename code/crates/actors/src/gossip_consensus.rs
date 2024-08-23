use std::collections::BTreeSet;
use std::marker::PhantomData;

use async_trait::async_trait;
use derive_where::derive_where;
use libp2p::identity::Keypair;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::task::JoinHandle;
use tracing::{debug, error, error_span, Instrument};

use malachite_common::{Context, ProposalPart, SignedProposal, SignedProposalPart, SignedVote};
use malachite_consensus::GossipMsg;
use malachite_gossip_consensus::handle::CtrlHandle;
use malachite_gossip_consensus::{Channel, Config, Event, Multiaddr, PeerId};
use malachite_metrics::SharedRegistry;
use malachite_proto::Protobuf;

use crate::util::codec::NetworkCodec;
use crate::util::streaming::{StreamContent, StreamMessage};

pub type GossipConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

#[derive_where(Default)]
pub struct GossipConsensus<Ctx, Codec> {
    marker: PhantomData<(Ctx, Codec)>,
}

impl<Ctx, Codec> GossipConsensus<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<Ctx>,
    Ctx::ProposalPart: Protobuf,
{
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
        codec: Codec,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let args = Args {
            keypair,
            config,
            metrics,
            codec,
        };

        let (actor_ref, _) = if let Some(supervisor) = supervisor {
            Actor::spawn_linked(None, Self::default(), args, supervisor).await?
        } else {
            Actor::spawn(None, Self::default(), args).await?
        };

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

pub struct Args<Codec> {
    pub keypair: Keypair,
    pub config: Config,
    pub metrics: SharedRegistry,
    pub codec: Codec,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum GossipEvent<Ctx: Context> {
    Listening(Multiaddr),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    Vote(PeerId, SignedVote<Ctx>),
    Proposal(PeerId, SignedProposal<Ctx>),
    ProposalPart(PeerId, StreamMessage<Ctx::ProposalPart>),
}

#[derive(Default)]
pub struct OutgoingStream {
    pub stream_id: u64,
    pub sequence: u64,
}

pub enum State<Ctx: Context> {
    Stopped,
    Running {
        peers: BTreeSet<PeerId>,
        subscribers: Vec<ActorRef<GossipEvent<Ctx>>>,
        outgoing_stream: OutgoingStream,
        ctrl_handle: CtrlHandle,
        recv_task: JoinHandle<()>,
        marker: PhantomData<Ctx>,
    },
}

pub enum Msg<Ctx: Context> {
    /// Subscribe this actor to receive gossip events
    Subscribe(ActorRef<GossipEvent<Ctx>>),

    /// Broadcast a gossip message
    BroadcastMsg(GossipMsg<Ctx>),

    /// Broadcast a proposal part
    BroadcastProposalPart(SignedProposalPart<Ctx>),

    /// Request for number of peers from gossip
    GetState { reply: RpcReplyPort<usize> },

    // Internal message
    #[doc(hidden)]
    NewEvent(Event),
}

#[async_trait]
impl<Ctx, Codec> Actor for GossipConsensus<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<Ctx>,
    Ctx::ProposalPart: Protobuf,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args<Codec>;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        args: Args<Codec>,
    ) -> Result<Self::State, ActorProcessingErr> {
        let handle =
            malachite_gossip_consensus::spawn(args.keypair, args.config, args.metrics).await?;

        let (mut recv_handle, ctrl_handle) = handle.split();

        let recv_task = tokio::spawn(
            async move {
                while let Some(event) = recv_handle.recv().await {
                    if let Err(e) = myself.cast(Msg::NewEvent(event)) {
                        error!("Actor has died, stopping gossip consensus: {e:?}");
                        break;
                    }
                }
            }
            .instrument(error_span!("gossip.consensus")),
        );

        Ok(State::Running {
            peers: BTreeSet::new(),
            subscribers: Vec::new(),
            outgoing_stream: OutgoingStream::default(),
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

    #[tracing::instrument(name = "gossip.consensus", skip(self, _myself, msg, state))]
    async fn handle(
        &self,
        _myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        let State::Running {
            peers,
            subscribers,
            outgoing_stream,
            ctrl_handle,
            ..
        } = state
        else {
            return Ok(());
        };

        match msg {
            Msg::Subscribe(subscriber) => subscribers.push(subscriber),

            Msg::BroadcastMsg(msg) => match Codec::encode_msg(msg) {
                Ok(data) => ctrl_handle.broadcast(Channel::Consensus, data).await?,
                Err(e) => error!("Failed to encode gossip message: {e:?}"),
            },

            Msg::BroadcastProposalPart(part) => {
                if part.message.is_first() {
                    outgoing_stream.stream_id += 1;
                    outgoing_stream.sequence = 0;
                }

                let is_last = part.message.is_last();

                debug!(
                    is_first = %part.message.is_first(),
                    is_last = %is_last,
                    stream_id = %outgoing_stream.stream_id,
                    sequence = %outgoing_stream.sequence,
                    "Broadcasting proposal part"
                );

                let data = Codec::encode_stream_msg::<Ctx::ProposalPart>(StreamMessage {
                    stream_id: outgoing_stream.stream_id,
                    sequence: outgoing_stream.sequence,
                    content: StreamContent::Data(part.message),
                });

                match data {
                    Ok(data) => {
                        ctrl_handle.broadcast(Channel::ProposalParts, data).await?;
                        outgoing_stream.sequence += 1;
                    }
                    Err(e) => error!("Failed to encode proposal part: {e:?}"),
                }

                if is_last {
                    let data = Codec::encode_stream_msg::<Ctx::ProposalPart>(StreamMessage {
                        stream_id: outgoing_stream.stream_id,
                        sequence: outgoing_stream.sequence,
                        content: StreamContent::Fin(true),
                    });

                    match data {
                        Ok(data) => {
                            ctrl_handle.broadcast(Channel::ProposalParts, data).await?;
                        }
                        Err(e) => error!("Failed to encode proposal part: {e:?}"),
                    }
                }
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

            Msg::NewEvent(Event::Message(Channel::Consensus, from, msg_id, data)) => {
                let msg = match Codec::decode_msg(data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(%from, "Failed to decode gossip message {msg_id}: {e:?}");
                        return Ok(());
                    }
                };

                let event = match msg {
                    GossipMsg::Vote(vote) => GossipEvent::Vote(from, vote),
                    GossipMsg::Proposal(proposal) => GossipEvent::Proposal(from, proposal),
                };

                self.publish(event, subscribers);
            }

            Msg::NewEvent(Event::Message(Channel::ProposalParts, from, msg_id, data)) => {
                let msg = match Codec::decode_stream_msg::<Ctx::ProposalPart>(data) {
                    Ok(stream_msg) => stream_msg,
                    Err(e) => {
                        error!(%from, %msg_id, "Failed to decode stream message: {e:?}");
                        return Ok(());
                    }
                };

                debug!(
                    %from,
                    stream_id = %msg.stream_id,
                    sequence = %msg.sequence,
                    "Received proposal part"
                );

                self.publish(GossipEvent::ProposalPart(from, msg), subscribers);
            }

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

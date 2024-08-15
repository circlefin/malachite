use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use derive_where::derive_where;
use libp2p::identity::Keypair;
use ractor::ActorCell;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use ractor::{Actor, RpcReplyPort};
use tokio::task::JoinHandle;

use malachite_common::Context;
use malachite_consensus::GossipMsg;
use malachite_gossip_consensus::handle::CtrlHandle;
use malachite_gossip_consensus::{Channel, Config, Event, NetworkCodec, PeerId};
use malachite_metrics::SharedRegistry;
use tracing::{error, error_span, Instrument};

pub type GossipConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

#[derive_where(Default)]
pub struct GossipConsensus<Ctx, Codec> {
    marker: PhantomData<(Ctx, Codec)>,
}

impl<Ctx: Context, Codec> GossipConsensus<Ctx, Codec> {
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
        codec: Codec,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr>
    where
        Codec: NetworkCodec<Ctx> + Send + Sync + 'static,
    {
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
}

pub struct Args<Codec> {
    pub keypair: Keypair,
    pub config: Config,
    pub metrics: SharedRegistry,
    pub codec: Codec,
}

pub enum State<Ctx: Context> {
    Stopped,
    Running {
        peers: BTreeSet<PeerId>,
        subscribers: Vec<ActorRef<Arc<Event<Ctx>>>>,
        ctrl_handle: CtrlHandle<Ctx>,
        recv_task: JoinHandle<()>,
    },
}

pub enum Msg<Ctx: Context> {
    Subscribe(ActorRef<Arc<Event<Ctx>>>),
    Broadcast(Channel, GossipMsg<Ctx>),

    // Internal message
    #[doc(hidden)]
    NewEvent(Event<Ctx>),
    // Request for number of peers from gossip
    GetState {
        reply: RpcReplyPort<usize>,
    },
}

#[async_trait]
impl<Ctx: Context, Codec> Actor for GossipConsensus<Ctx, Codec>
where
    Codec: NetworkCodec<Ctx> + Send + Sync + 'static,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args<Codec>;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        args: Args<Codec>,
    ) -> Result<Self::State, ActorProcessingErr> {
        let handle = malachite_gossip_consensus::spawn::<Ctx>(
            args.keypair,
            args.config,
            args.codec,
            args.metrics,
        )
        .await?;

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
            ctrl_handle,
            recv_task,
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
            ctrl_handle,
            ..
        } = state
        else {
            return Ok(());
        };

        match msg {
            Msg::Subscribe(subscriber) => subscribers.push(subscriber),
            Msg::Broadcast(channel, data) => ctrl_handle.broadcast(channel, data).await?,
            Msg::NewEvent(event) => {
                match event {
                    Event::PeerConnected(peer_id) => {
                        peers.insert(peer_id);
                    }
                    Event::PeerDisconnected(peer_id) => {
                        peers.remove(&peer_id);
                    }
                    _ => {}
                }

                let event = Arc::new(event);
                for subscriber in subscribers {
                    subscriber.cast(Arc::clone(&event))?;
                }
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

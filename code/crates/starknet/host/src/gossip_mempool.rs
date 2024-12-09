use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use libp2p_identity::Keypair;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use ractor::{Actor, RpcReplyPort};
use tokio::task::JoinHandle;
use tracing::error;

use malachite_gossip_mempool::handle::CtrlHandle;
use malachite_gossip_mempool::types::MempoolTransactionBatch;
use malachite_gossip_mempool::Channel::Mempool;
use malachite_gossip_mempool::{Config, Event, NetworkMsg, PeerId};
use malachite_metrics::SharedRegistry;

pub type GossipMempoolRef = ActorRef<Msg>;

pub struct GossipMempool;

impl GossipMempool {
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
    ) -> Result<ActorRef<Msg>, ractor::SpawnErr> {
        let args = Args {
            keypair,
            config,
            metrics,
        };

        let (actor_ref, _) = Actor::spawn(None, Self, args).await?;
        Ok(actor_ref)
    }
}

pub struct Args {
    pub keypair: Keypair,
    pub config: Config,
    pub metrics: SharedRegistry,
}

pub enum State {
    Stopped,
    Running {
        peers: BTreeSet<PeerId>,
        subscribers: Vec<ActorRef<Arc<Event>>>,
        ctrl_handle: CtrlHandle,
        recv_task: JoinHandle<()>,
    },
}

pub enum Msg {
    /// Subscribe to gossip events
    Subscribe(ActorRef<Arc<Event>>),

    /// Broadcast a message to all peers
    BroadcastMsg(MempoolTransactionBatch),

    /// Request the number of connected peers
    GetState { reply: RpcReplyPort<usize> },

    // Internal message
    #[doc(hidden)]
    NewEvent(Event),
}

#[async_trait]
impl Actor for GossipMempool {
    type Msg = Msg;
    type State = State;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg>,
        args: Args,
    ) -> Result<State, ActorProcessingErr> {
        let handle =
            malachite_gossip_mempool::spawn(args.keypair, args.config, args.metrics).await?;
        let (mut recv_handle, ctrl_handle) = handle.split();

        let recv_task = tokio::spawn(async move {
            while let Some(event) = recv_handle.recv().await {
                if let Err(e) = myself.cast(Msg::NewEvent(event)) {
                    error!("Actor has died, stopping gossip mempool: {e:?}");
                    break;
                }
            }
        });

        Ok(State::Running {
            peers: BTreeSet::new(),
            subscribers: Vec::new(),
            ctrl_handle,
            recv_task,
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<Msg>,
        _state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
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
            Msg::BroadcastMsg(batch) => {
                match NetworkMsg::TransactionBatch(batch).to_network_bytes() {
                    Ok(bytes) => {
                        ctrl_handle.broadcast(Mempool, bytes).await?;
                    }
                    Err(e) => {
                        error!("Failed to serialize transaction batch: {e}");
                    }
                }
            }
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
        _myself: ActorRef<Msg>,
        state: &mut State,
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

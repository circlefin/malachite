use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use libp2p_identity::Keypair;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use ractor::{Actor, RpcReplyPort};
use tokio::task::JoinHandle;
use tracing::error;

use malachitebft_engine::util::output_port::{OutputPort, OutputPortSubscriber};
use malachitebft_metrics::SharedRegistry;
use malachitebft_test_mempool::handle::CtrlHandle;
use malachitebft_test_mempool::types::MempoolTransactionBatch;
use malachitebft_test_mempool::Channel::Mempool;
use malachitebft_test_mempool::{Config, Event, NetworkMsg, PeerId};

pub type MempoolNetworkMsg = Msg;
pub type MempoolNetworkRef = ActorRef<Msg>;

pub struct MempoolNetwork {
    span: tracing::Span,
}

impl MempoolNetwork {
    pub async fn spawn(
        keypair: Keypair,
        config: Config,
        metrics: SharedRegistry,
        span: tracing::Span,
    ) -> Result<ActorRef<Msg>, ractor::SpawnErr> {
        let args = Args {
            keypair,
            config,
            metrics,
        };

        let (actor_ref, _) = Actor::spawn(None, Self { span }, args).await?;
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
        output_port: OutputPort<Arc<Event>>,
        ctrl_handle: CtrlHandle,
        recv_task: JoinHandle<()>,
    },
}

pub enum Msg {
    /// Subscribe to gossip events
    Subscribe(OutputPortSubscriber<Arc<Event>>),

    /// Broadcast a message to all peers
    BroadcastMsg(MempoolTransactionBatch),

    /// Request the number of connected peers
    GetState { reply: RpcReplyPort<usize> },

    // Internal message
    #[doc(hidden)]
    NewEvent(Event),
}

#[async_trait]
impl Actor for MempoolNetwork {
    type Msg = Msg;
    type State = State;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Msg>,
        args: Args,
    ) -> Result<State, ActorProcessingErr> {
        let handle =
            malachitebft_test_mempool::spawn(args.keypair, args.config, args.metrics).await?;
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
            output_port: OutputPort::default(),
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

    #[tracing::instrument(name = "gossip.mempool", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        let State::Running {
            peers,
            output_port,
            ctrl_handle,
            ..
        } = state
        else {
            return Ok(());
        };

        match msg {
            Msg::Subscribe(subscriber) => subscriber.subscribe_to_port(output_port),

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
                output_port.send(event);
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

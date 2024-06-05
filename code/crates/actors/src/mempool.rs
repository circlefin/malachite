use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use bytesize::ByteSize;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::distributions::Uniform;
use rand::Rng;
use tracing::{debug, info};

use malachite_common::Transaction;
use malachite_gossip_mempool::{Channel, Event as GossipEvent, PeerId};

use crate::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use crate::util::forward;

#[derive(Clone, Debug, PartialEq)]
pub enum NetworkMsg {
    Transaction(Vec<u8>),
}

impl NetworkMsg {
    pub fn from_network_bytes(bytes: &[u8]) -> Self {
        NetworkMsg::Transaction(bytes.to_vec())
    }

    pub fn to_network_bytes(&self) -> Vec<u8> {
        match self {
            NetworkMsg::Transaction(bytes) => bytes.to_vec(),
        }
    }
}

pub enum Next {
    None,
    Transaction(Transaction),
}

pub type MempoolRef = ActorRef<Msg>;

pub struct Mempool {
    gossip_mempool: GossipMempoolRef,
}

pub enum Msg {
    GossipEvent(Arc<GossipEvent>),
    Input(Transaction),
    TxStream {
        height: u64,
        tx_size: ByteSize,
        num_txes: u64,
        reply: RpcReplyPort<Vec<Transaction>>,
    },
}

#[allow(dead_code)]
pub struct State {
    msg_queue: VecDeque<Msg>,
    transactions: Vec<Transaction>,
}

impl Mempool {
    pub fn new(gossip_mempool: GossipMempoolRef) -> Self {
        Self { gossip_mempool }
    }

    pub async fn spawn(
        gossip_mempool: GossipMempoolRef,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg>, ractor::SpawnErr> {
        let node = Self::new(gossip_mempool);

        let (actor_ref, _) = if let Some(supervisor) = supervisor {
            Actor::spawn_linked(None, node, (), supervisor).await?
        } else {
            Actor::spawn(None, node, ()).await?
        };

        Ok(actor_ref)
    }

    pub async fn handle_gossip_event(
        &self,
        event: &GossipEvent,
        myself: ActorRef<Msg>,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match event {
            GossipEvent::Listening(addr) => {
                info!("Listening on {addr}");
            }
            GossipEvent::PeerConnected(peer_id) => {
                info!("Connected to peer {peer_id}");
            }
            GossipEvent::PeerDisconnected(peer_id) => {
                info!("Disconnected from peer {peer_id}");
            }
            GossipEvent::Message(from, Channel::Mempool, data) => {
                let msg = NetworkMsg::from_network_bytes(data);

                debug!("Mempool - Received message from peer {from}: {msg:?}");

                self.handle_network_msg(from, msg, myself, state).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: &PeerId,
        msg: NetworkMsg,
        myself: ActorRef<Msg>,
        _state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            NetworkMsg::Transaction(bytes) => {
                info!(%from, "Received transaction: {:?}", bytes);

                myself.cast(Msg::Input(Transaction::new(bytes)))?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for Mempool {
    type Msg = Msg;
    type State = State;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Msg>,
        _args: (),
    ) -> Result<State, ractor::ActorProcessingErr> {
        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;
        self.gossip_mempool
            .cast(GossipMempoolMsg::Subscribe(forward))?;

        Ok(State {
            msg_queue: VecDeque::new(),
            transactions: vec![],
        })
    }

    #[tracing::instrument(name = "node", skip(self, myself, msg, state))]
    async fn handle(
        &self,
        myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            Msg::GossipEvent(event) => {
                self.handle_gossip_event(&event, myself, state).await?;
            }

            Msg::Input(tx) => {
                state.transactions.push(tx);
            }

            Msg::TxStream {
                reply,
                tx_size,
                num_txes,
                ..
            } => {
                let mut transactions = vec![];

                let mut rng = rand::thread_rng();
                for _i in 0..num_txes {
                    // Generate transaction
                    let range = Uniform::new(32, 64);
                    let tx: Vec<u8> = (0..tx_size.as_u64()).map(|_| rng.sample(range)).collect();
                    // TODO - Gossip, remove on decided block
                    // let msg = NetworkMsg::Transaction(tx.clone());
                    // let bytes = msg.to_network_bytes();
                    // self.gossip_mempool
                    //     .cast(GossipMempoolMsg::Broadcast(Channel::Mempool, bytes))?;
                    transactions.push(Transaction::new(tx));
                }

                reply.send(transactions)?;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        _state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        info!("Stopping...");

        Ok(())
    }
}

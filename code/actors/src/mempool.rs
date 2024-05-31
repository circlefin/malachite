use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::Rng;
use tracing::{debug, info};

use malachite_common::Transaction;
use malachite_gossip_mempool::{Channel, Event as GossipEvent, Event};
use malachite_network_mempool::{Msg as NetworkMsg, PeerId};

use crate::gossip_mempool::Msg as GossipMsg;
use crate::util::forward;

pub enum Next {
    None,
    Transaction(Transaction),
}

pub struct Params {}

#[allow(dead_code)]
pub struct Mempool {
    params: Params,
    gossip: ActorRef<GossipMsg>,
}

pub enum Msg {
    Start,
    GossipEvent(Arc<GossipEvent>),
    Input(Transaction),
    TxStream {
        height: u64,
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
    pub fn new(params: Params, gossip: ActorRef<GossipMsg>) -> Self {
        Self { params, gossip }
    }

    pub async fn spawn(
        params: Params,
        gossip: ActorRef<GossipMsg>,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg>, ractor::SpawnErr> {
        let node = Self::new(params, gossip);

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
                let from = PeerId::new(from.to_string());
                let msg = NetworkMsg::from_network_bytes(data);

                debug!("Mempool - Received message from peer {from}: {msg:?}");

                self.handle_network_msg(from, msg, myself, state).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: PeerId,
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

    pub async fn send_input(
        &self,
        input: Transaction,
        state: &mut crate::mempool::State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        state.transactions.push(input);
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
        self.gossip.cast(GossipMsg::Subscribe(forward))?;

        let mut transactions = vec![];

        for _i in 0..2 {
            let bytes = rand::thread_rng().gen::<[u8; 4]>();
            transactions.push(Transaction::new(bytes.into()));
        }

        info!("Generated mempool txes: {transactions:?}");

        Ok(State {
            msg_queue: VecDeque::new(),
            transactions,
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
                if let Event::Message(_, _, _) = event.as_ref() {
                    self.handle_gossip_event(event.as_ref(), myself, state)
                        .await?;
                }
            }

            Msg::Input(tx) => {
                self.send_input(tx, state).await?;
            }

            Msg::Start => {
                for tx in state.transactions.iter() {
                    let msg = NetworkMsg::Transaction(tx.to_bytes());
                    let bytes = msg.to_network_bytes();
                    self.gossip
                        .cast(GossipMsg::Broadcast(Channel::Mempool, bytes))?;
                }
            }

            Msg::TxStream {
                reply, num_txes, ..
            } => {
                let txes_len = state.transactions.len();
                let mut txes = vec![];
                for _i in 0..num_txes as usize / txes_len {
                    txes.extend(state.transactions.clone());
                }
                reply.send(txes)?;
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

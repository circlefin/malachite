use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::distributions::Uniform;
use rand::Rng;
use tracing::{debug, info, trace};

use malachite_common::{MempoolTransactionBatch, Transaction, TransactionBatch};
use malachite_gossip_mempool::{Channel, Event as GossipEvent, PeerId};
use malachite_node::config::{MempoolConfig, TestConfig};
use malachite_proto::Protobuf;

use crate::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use crate::util::forward;

#[derive(Clone, Debug, PartialEq)]
pub enum NetworkMsg {
    TransactionBatch(MempoolTransactionBatch),
}

impl NetworkMsg {
    pub fn from_network_bytes(bytes: &[u8]) -> Self {
        let batch = Protobuf::from_bytes(bytes).unwrap(); // FIXME: Error handling
        NetworkMsg::TransactionBatch(batch)
    }

    pub fn to_network_bytes(&self) -> malachite_proto::MempoolTransactionBatch {
        match self {
            NetworkMsg::TransactionBatch(batch) => batch.to_proto().unwrap(), // FXME: Error handling
        }
    }
}

pub type MempoolRef = ActorRef<Msg>;

pub struct Mempool {
    gossip_mempool: GossipMempoolRef,
    mempool_config: MempoolConfig, // todo - pick only what's needed
    test_config: TestConfig,       // todo - pick only the mempool related
}

pub enum Msg {
    GossipEvent(Arc<GossipEvent>),
    Input(Transaction),
    TxStream {
        height: u64,
        num_txes: usize,
        reply: RpcReplyPort<Vec<Transaction>>,
    },
}

#[allow(dead_code)]
pub struct State {
    msg_queue: VecDeque<Msg>,
    transactions: Vec<Transaction>,
}

impl Mempool {
    pub fn new(
        gossip_mempool: GossipMempoolRef,
        mempool_config: MempoolConfig,
        test_config: TestConfig,
    ) -> Self {
        Self {
            gossip_mempool,
            mempool_config,
            test_config,
        }
    }

    pub async fn spawn(
        gossip_mempool: GossipMempoolRef,
        mempool_config: &MempoolConfig,
        test_config: &TestConfig,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg>, ractor::SpawnErr> {
        let node = Self::new(gossip_mempool, mempool_config.clone(), *test_config);

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
                trace!(%from, "Received message of size {} bytes", data.len());

                let msg = NetworkMsg::from_network_bytes(data);
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
            NetworkMsg::TransactionBatch(batch) => {
                debug!(%from, "Received batch with {} transactions", batch.len());

                for tx in batch.transaction_batch.into_transactions() {
                    myself.cast(Msg::Input(tx))?;
                }
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

    #[tracing::instrument(name = "mempool", skip(self, myself, msg, state))]
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
                if state.transactions.len() < self.mempool_config.max_tx_count {
                    state.transactions.push(tx);
                } else {
                    trace!("Mempool is full, dropping transaction");
                }
            }

            Msg::TxStream {
                reply, num_txes, ..
            } => {
                let txes = generate_txes(
                    num_txes,
                    self.test_config.tx_size.as_u64(),
                    self.mempool_config.gossip_batch_size,
                    &self.gossip_mempool,
                )?;

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

fn generate_txes(
    count: usize,
    size: u64,
    batch_size: usize,
    gossip_mempool: &GossipMempoolRef,
) -> Result<Vec<Transaction>, ActorProcessingErr> {
    let mut transactions = vec![];
    let mut tx_batch = TransactionBatch::default();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        // Generate transaction
        let range = Uniform::new(32, 64);
        let tx_bytes: Vec<u8> = (0..size).map(|_| rng.sample(range)).collect();
        let tx = Transaction::new(tx_bytes);

        // TODO: Remove tx-es on decided block
        tx_batch.push(tx.clone());

        if batch_size > 0 && tx_batch.len() >= batch_size {
            let mempool_batch = MempoolTransactionBatch::new(std::mem::take(&mut tx_batch));
            gossip_mempool.cast(GossipMempoolMsg::Broadcast(Channel::Mempool, mempool_batch))?;
        }

        transactions.push(tx);
    }

    Ok(transactions)
}

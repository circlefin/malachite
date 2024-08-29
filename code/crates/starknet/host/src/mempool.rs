use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::RngCore;
use tracing::{debug, info, trace};

use malachite_actors::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use malachite_actors::util::forward::forward;
use malachite_gossip_mempool::types::MempoolTransactionBatch;
use malachite_gossip_mempool::{Channel, Event as GossipEvent, NetworkMsg, PeerId};
use malachite_node::config::{MempoolConfig, TestConfig};
use malachite_proto::Protobuf;

use crate::types::{Hash, Transaction, Transactions};

pub type MempoolRef = ActorRef<MempoolMsg>;

pub struct Mempool {
    gossip_mempool: GossipMempoolRef,
    mempool_config: MempoolConfig, // todo - pick only what's needed
    test_config: TestConfig,       // todo - pick only the mempool related
}

pub enum MempoolMsg {
    GossipEvent(Arc<GossipEvent>),
    Input(Transaction),
    Reap {
        height: u64,
        num_txes: usize,
        reply: RpcReplyPort<Vec<Transaction>>,
    },
    Update {
        tx_hashes: Vec<Hash>,
    },
}

#[allow(dead_code)]
pub struct State {
    pub msg_queue: VecDeque<MempoolMsg>,
    pub transactions: BTreeMap<Hash, Transaction>,
}

impl State {
    pub fn new() -> Self {
        Self {
            msg_queue: VecDeque::new(),
            transactions: BTreeMap::new(),
        }
    }

    pub fn add_tx(&mut self, tx: &Transaction) {
        self.transactions.entry(tx.hash()).or_insert(tx.clone());
    }

    pub fn remove_tx(&mut self, hash: &Hash) {
        self.transactions.remove_entry(hash);
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
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
    ) -> Result<MempoolRef, ractor::SpawnErr> {
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
        myself: MempoolRef,
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
            GossipEvent::Message(from, msg) => {
                trace!(%from, "Received message of size {} bytes", msg.size_bytes());

                trace!(%from, "Received message");
                self.handle_network_msg(from, msg.clone(), myself, state) // FIXME: Clone
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: &PeerId,
        msg: NetworkMsg,
        myself: MempoolRef,
        _state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            NetworkMsg::TransactionBatch(batch) => {
                let Ok(batch) = Transactions::from_any(&batch.transaction_batch) else {
                    // TODO: Log error
                    return Ok(());
                };

                trace!(%from, "Received batch with {} transactions", batch.len());

                for tx in batch.into_vec() {
                    myself.cast(MempoolMsg::Input(tx))?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for Mempool {
    type Msg = MempoolMsg;
    type State = State;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: MempoolRef,
        _args: (),
    ) -> Result<State, ractor::ActorProcessingErr> {
        let forward = forward(
            myself.clone(),
            Some(myself.get_cell()),
            MempoolMsg::GossipEvent,
        )
        .await?;
        self.gossip_mempool
            .cast(GossipMempoolMsg::Subscribe(forward))?;

        Ok(State::new())
    }

    #[tracing::instrument("starknet.mempool", skip(self, myself, msg, state))]
    async fn handle(
        &self,
        myself: MempoolRef,
        msg: MempoolMsg,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            MempoolMsg::GossipEvent(event) => {
                self.handle_gossip_event(&event, myself, state).await?;
            }

            MempoolMsg::Input(tx) => {
                if state.transactions.len() < self.mempool_config.max_tx_count {
                    state.add_tx(&tx);
                } else {
                    trace!("Mempool is full, dropping transaction");
                }
            }

            MempoolMsg::Reap {
                reply, num_txes, ..
            } => {
                let txes = generate_and_broadcast_txes(
                    num_txes,
                    self.test_config.tx_size.as_u64() as usize,
                    &self.mempool_config,
                    state,
                    &self.gossip_mempool,
                )?;

                reply.send(txes)?;
            }

            MempoolMsg::Update { .. } => {
                // FIXME: Remove only the given txes
                // tx_hashes.iter().for_each(|hash| state.remove_tx(hash));

                state.transactions.clear();
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

fn generate_and_broadcast_txes(
    count: usize,
    size: usize,
    config: &MempoolConfig,
    state: &mut State,
    gossip_mempool: &GossipMempoolRef,
) -> Result<Vec<Transaction>, ActorProcessingErr> {
    debug!("Generating {} transactions of size {} bytes", count, size);

    let batch_size = std::cmp::min(config.gossip_batch_size, count);

    let mut transactions = vec![];
    let mut tx_batch = Transactions::default();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        // Generate transaction
        let mut tx_bytes = vec![0; size];
        rng.fill_bytes(&mut tx_bytes);
        let tx = Transaction::new(tx_bytes);

        // Add transaction to state
        if state.transactions.len() < config.max_tx_count {
            state.add_tx(&tx);
        }

        tx_batch.push(tx.clone());

        // Gossip tx-es to peers in batches
        if config.gossip_batch_size > 0 && tx_batch.len() >= batch_size {
            let tx_batch = std::mem::take(&mut tx_batch);

            let Ok(tx_batch_any) = tx_batch.to_any() else {
                // TODO: Handle error
                continue;
            };

            let mempool_batch = MempoolTransactionBatch::new(tx_batch_any);
            gossip_mempool.cast(GossipMempoolMsg::Broadcast(Channel::Mempool, mempool_batch))?;
        }

        transactions.push(tx);
    }

    Ok(transactions)
}

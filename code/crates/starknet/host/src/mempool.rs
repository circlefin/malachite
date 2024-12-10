use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::RngCore;
use tracing::{debug, info, trace};

use malachite_actors::util::forward::forward;
use malachite_config::{MempoolConfig, TestConfig};
use malachite_test_mempool::types::MempoolTransactionBatch;
use malachite_test_mempool::{Event as GossipEvent, NetworkMsg, PeerId};

use crate::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use crate::proto::Protobuf;
use crate::types::{Hash, Transaction, Transactions};

pub type MempoolRef = ActorRef<MempoolMsg>;

pub struct Mempool {
    gossip_mempool: GossipMempoolRef,
    mempool_config: MempoolConfig, // todo - pick only what's needed
    test_config: TestConfig,       // todo - pick only the mempool related
    span: tracing::Span,
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
    pub transactions: BTreeMap<Hash, Transaction>,
}

impl State {
    pub fn new() -> Self {
        Self {
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
        span: tracing::Span,
    ) -> Self {
        Self {
            gossip_mempool,
            mempool_config,
            test_config,
            span,
        }
    }

    pub async fn spawn(
        gossip_mempool: GossipMempoolRef,
        mempool_config: MempoolConfig,
        test_config: TestConfig,
        span: tracing::Span,
    ) -> Result<MempoolRef, ractor::SpawnErr> {
        let node = Self::new(gossip_mempool, mempool_config, test_config, span);

        let (actor_ref, _) = Actor::spawn(None, node, ()).await?;
        Ok(actor_ref)
    }

    pub async fn handle_gossip_event(
        &self,
        event: &GossipEvent,
        myself: MempoolRef,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match event {
            GossipEvent::Listening(address) => {
                info!(%address, "Listening");
            }
            GossipEvent::PeerConnected(peer_id) => {
                info!(%peer_id, "Connected to peer");
            }
            GossipEvent::PeerDisconnected(peer_id) => {
                info!(%peer_id, "Disconnected from peer");
            }
            GossipEvent::Message(_channel, from, _msg_id, msg) => {
                trace!(%from, size = msg.size_bytes(), "Received message");

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

        self.gossip_mempool.link(myself.get_cell());

        self.gossip_mempool
            .cast(GossipMempoolMsg::Subscribe(forward))?;

        Ok(State::new())
    }

    #[tracing::instrument("host.mempool", parent = &self.span, skip_all)]
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
    _state: &mut State,
    gossip_mempool: &GossipMempoolRef,
) -> Result<Vec<Transaction>, ActorProcessingErr> {
    debug!(%count, %size, "Generating transactions");

    let batch_size = std::cmp::min(config.gossip_batch_size, count);
    let gossip_enabled = config.gossip_batch_size > 0;

    let mut transactions = Vec::with_capacity(count);
    let mut tx_batch = Transactions::default();
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        // Generate transaction
        let mut tx_bytes = vec![0; size];
        rng.fill_bytes(&mut tx_bytes);
        let tx = Transaction::new(tx_bytes);

        if gossip_enabled {
            tx_batch.push(tx.clone());
        }

        transactions.push(tx);

        // if state.transactions.len() < config.max_tx_count {
        //     state.add_tx(&tx);
        // }

        // Gossip tx-es to peers in batches
        if gossip_enabled && tx_batch.len() >= batch_size {
            let tx_batch = std::mem::take(&mut tx_batch);

            let Ok(tx_batch_any) = tx_batch.to_any() else {
                // TODO: Handle error
                continue;
            };

            let mempool_batch = MempoolTransactionBatch::new(tx_batch_any);
            gossip_mempool.cast(GossipMempoolMsg::BroadcastMsg(mempool_batch))?;
        }
    }

    Ok(transactions)
}

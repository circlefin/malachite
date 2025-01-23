use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytesize::ByteSize;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use rand::RngCore;
use tracing::{debug, info, trace};

use malachitebft_test_mempool::types::MempoolTransactionBatch;
use malachitebft_test_mempool::{Event as NetworkEvent, NetworkMsg, PeerId};

use crate::proto::Protobuf;
use crate::types::{Hash, Transaction, Transactions};

pub mod network;
use network::{MempoolNetworkMsg, MempoolNetworkRef};

pub type MempoolMsg = Msg;
pub type MempoolRef = ActorRef<Msg>;

pub struct Mempool {
    network: MempoolNetworkRef,
    gossip_batch_size: usize,
    max_tx_count: usize,
    test_tx_size: ByteSize,
    span: tracing::Span,
}

pub enum Msg {
    NetworkEvent(Arc<NetworkEvent>),
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

impl From<Arc<NetworkEvent>> for Msg {
    fn from(event: Arc<NetworkEvent>) -> Self {
        Self::NetworkEvent(event)
    }
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
        mempool_network: MempoolNetworkRef,
        gossip_batch_size: usize,
        max_tx_count: usize,
        test_tx_size: ByteSize,
        span: tracing::Span,
    ) -> Self {
        Self {
            network: mempool_network,
            gossip_batch_size,
            max_tx_count,
            test_tx_size,
            span,
        }
    }

    pub async fn spawn(
        mempool_network: MempoolNetworkRef,
        gossip_batch_size: usize,
        max_tx_count: usize,
        test_tx_size: ByteSize,
        span: tracing::Span,
    ) -> Result<MempoolRef, ractor::SpawnErr> {
        let node = Self::new(
            mempool_network,
            gossip_batch_size,
            max_tx_count,
            test_tx_size,
            span,
        );

        let (actor_ref, _) = Actor::spawn(None, node, ()).await?;
        Ok(actor_ref)
    }

    pub async fn handle_network_event(
        &self,
        event: &NetworkEvent,
        myself: MempoolRef,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match event {
            NetworkEvent::Listening(address) => {
                info!(%address, "Listening");
            }
            NetworkEvent::PeerConnected(peer_id) => {
                info!(%peer_id, "Connected to peer");
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                info!(%peer_id, "Disconnected from peer");
            }
            NetworkEvent::Message(_channel, from, _msg_id, msg) => {
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
        myself: MempoolRef,
        _args: (),
    ) -> Result<State, ractor::ActorProcessingErr> {
        self.network.link(myself.get_cell());

        self.network
            .cast(MempoolNetworkMsg::Subscribe(Box::new(myself.clone())))?;

        Ok(State::new())
    }

    #[tracing::instrument("host.mempool", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        myself: MempoolRef,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            Msg::NetworkEvent(event) => {
                self.handle_network_event(&event, myself, state).await?;
            }

            Msg::Input(tx) => {
                if state.transactions.len() < self.max_tx_count {
                    state.add_tx(&tx);
                } else {
                    trace!("Mempool is full, dropping transaction");
                }
            }

            Msg::Reap {
                reply, num_txes, ..
            } => {
                let txes = generate_and_broadcast_txes(
                    num_txes,
                    self.test_tx_size.as_u64() as usize,
                    self.gossip_batch_size,
                    state,
                    &self.network,
                )?;

                reply.send(txes)?;
            }

            Msg::Update { .. } => {
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
    gossip_batch_size: usize,
    _state: &mut State,
    mempool_network: &MempoolNetworkRef,
) -> Result<Vec<Transaction>, ActorProcessingErr> {
    debug!(%count, %size, "Generating transactions");

    let batch_size = std::cmp::min(gossip_batch_size, count);
    let gossip_enabled = gossip_batch_size > 0;

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
            mempool_network.cast(MempoolNetworkMsg::BroadcastMsg(mempool_batch))?;
        }
    }

    Ok(transactions)
}

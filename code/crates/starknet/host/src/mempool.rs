use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use itertools::Itertools;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tracing::{debug, error, info, trace};

use malachitebft_test_mempool::types::MempoolTransactionBatch;
use malachitebft_test_mempool::{Event as NetworkEvent, NetworkMsg, PeerId};

use crate::proto::Protobuf;
use crate::types::{Hash, Transaction, TransactionBatch};

pub mod network;
use network::{MempoolNetworkMsg, MempoolNetworkRef};

pub type MempoolMsg = Msg;
pub type MempoolRef = ActorRef<Msg>;

pub struct Mempool {
    network: MempoolNetworkRef,
    gossip_batch_size: usize,
    max_tx_count: usize,
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

#[derive(Default)]
pub struct State {
    transactions: BTreeMap<Hash, Transaction>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tx(&mut self, tx: Transaction) {
        self.transactions.entry(tx.hash()).or_insert(tx);
    }

    pub fn remove_tx(&mut self, hash: &Hash) {
        self.transactions.remove(hash);
    }
}

impl Mempool {
    pub fn new(
        mempool_network: MempoolNetworkRef,
        gossip_batch_size: usize,
        max_tx_count: usize,
        span: tracing::Span,
    ) -> Self {
        Self {
            network: mempool_network,
            gossip_batch_size,
            max_tx_count,
            span,
        }
    }

    pub async fn spawn(
        mempool_network: MempoolNetworkRef,
        gossip_batch_size: usize,
        max_tx_count: usize,
        span: tracing::Span,
    ) -> Result<MempoolRef, ractor::SpawnErr> {
        let node = Self::new(mempool_network, gossip_batch_size, max_tx_count, span);
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

                self.handle_network_msg(from, msg, myself, state).await?;
            }
        }

        Ok(())
    }

    pub async fn handle_network_msg(
        &self,
        from: &PeerId,
        msg: &NetworkMsg,
        myself: MempoolRef,
        _state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        match msg {
            NetworkMsg::TransactionBatch(batch) => {
                let batch = match TransactionBatch::from_any(&batch.transaction_batch) {
                    Ok(batch) => batch,
                    Err(e) => {
                        error!("Failed to decode transaction batch: {e}");
                        return Ok(());
                    }
                };

                trace!(%from, "Received batch with {} transactions", batch.len());

                for tx in batch.into_vec() {
                    myself.cast(Msg::Input(tx))?;
                }
            }
        }

        Ok(())
    }

    async fn handle_msg(
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
                    state.add_tx(tx);
                } else {
                    trace!("Mempool is full, dropping transaction");
                }
            }

            Msg::Reap {
                reply, num_txes, ..
            } => {
                let txes = reap_and_broadcast_txes(
                    num_txes,
                    self.gossip_batch_size,
                    state,
                    &self.network,
                )?;

                reply.send(txes)?;
            }

            Msg::Update { tx_hashes } => {
                tx_hashes.iter().for_each(|hash| state.remove_tx(hash));
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

    #[tracing::instrument("host.mempool", parent = &self.span, skip_all)]
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
        msg: MempoolMsg,
        state: &mut State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        if let Err(e) = self.handle_msg(myself, msg, state).await {
            error!("Error processing message: {e:?}");
        }

        Ok(())
    }

    #[tracing::instrument("host.mempool", parent = &self.span, skip_all)]
    async fn post_stop(
        &self,
        _myself: MempoolRef,
        _state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        info!("Stopping...");

        Ok(())
    }
}

fn reap_and_broadcast_txes(
    count: usize,
    gossip_batch_size: usize,
    state: &mut State,
    mempool_network: &MempoolNetworkRef,
) -> Result<Vec<Transaction>, ActorProcessingErr> {
    debug!(%count, "Reaping transactions");

    let gossip_enabled = gossip_batch_size > 0;

    // Reap transactions from the mempool
    let transactions = std::mem::take(&mut state.transactions)
        .into_values()
        .take(count)
        .collect::<Vec<_>>();

    // If mempool gossip is enabled, broadcast the transactions to the network
    if gossip_enabled {
        // Chunk the transactions in batch of max `gossip_batch_size`
        for batch in &transactions.iter().chunks(gossip_batch_size) {
            let tx_batch = TransactionBatch::new(batch.cloned().collect());
            let tx_batch = tx_batch.to_any().unwrap();
            let mempool_batch = MempoolTransactionBatch::new(tx_batch);
            mempool_network.cast(MempoolNetworkMsg::BroadcastMsg(mempool_batch))?;
        }
    }

    Ok(transactions)
}

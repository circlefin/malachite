// Adi: This is a copy of `mempool` module from `malachite_actors`
// with a single modification: it does not use the method
// `generate_and_broadcast_txes` to obtain transactions, but instead
// we get transactions from users via RPC calls

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use tracing::{info, trace};

use malachite_common::Transaction;
use malachite_gossip_mempool::{Event as GossipEvent, NetworkMsg, PeerId};
use malachite_node::config::{MempoolConfig, TestConfig};

use malachite_actors::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use malachite_actors::util::forward;

use malachite_actors::mempool::MempoolMsg;
use malachite_actors::mempool::MempoolRef;

#[allow(dead_code)]
pub struct KvMempool {
    gossip_mempool: GossipMempoolRef,
    mempool_config: MempoolConfig, // todo - pick only what's needed
    test_config: TestConfig,       // todo - pick only the mempool related
}

// Adi: We will not define our own MempoolMsg here
// Instead, we'll reuse the same messages from vanilla `malachite_actors`
// so that we can interface with the rest of the actors.
// pub enum MempoolMsg { ... }

#[allow(dead_code)]
pub struct State {
    pub msg_queue: VecDeque<MempoolMsg>,
    pub transactions: BTreeMap<u64, Transaction>,
}

impl State {
    pub fn new() -> Self {
        Self {
            msg_queue: VecDeque::new(),
            transactions: BTreeMap::new(),
        }
    }

    pub fn add_tx(&mut self, tx: &Transaction) {
        let mut hash = DefaultHasher::new();
        tx.0.hash(&mut hash);
        let key = hash.finish();
        self.transactions.entry(key).or_insert(tx.clone());
    }

    pub fn remove_tx(&mut self, hash: &u64) {
        self.transactions.remove_entry(hash);
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl KvMempool {
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
                // TODO: Implement Protobuf on NetworkMsg
                // trace!(%from, "Received message of size {} bytes", msg.encoded_len());
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
                trace!(%from, "Received batch with {} transactions", batch.len());

                for tx in batch.transaction_batch.into_transactions() {
                    myself.cast(MempoolMsg::Input(tx))?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for KvMempool {
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

    #[tracing::instrument(name = "mempool", skip(self, myself, msg, state))]
    async fn handle(
        &self,
        myself: MempoolRef,
        msg: MempoolMsg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
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

            // Adi: This is a request coming from the `Host` actor, specifically
            // from `build_new_proposal` via `run_build_proposal_task`
            MempoolMsg::TxStream {
                reply, ..
            } => {
                // let txes = generate_and_broadcast_txes(
                //     num_txes,
                //     self.test_config.tx_size.as_u64(),
                //     &self.mempool_config,
                //     state,
                //     &self.gossip_mempool,
                // )?;

                // TODO(Adi) Add proper generation code
                let txes = vec![];
                info!("Here we need to reap transactions; returning empty vector");

                reply.send(txes)?;
            }

            MempoolMsg::Update { .. } => {
                // tx_hashes.iter().for_each(|hash| state.remove_tx(hash));

                // FIXME: Reset the mempool for now
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

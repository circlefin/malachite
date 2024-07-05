// Adi: This is a copy of `mempool` module from `malachite_actors`
// with a single modification: it does not use the method
// `generate_and_broadcast_txes` to obtain transactions, but instead
// we get transactions from users via RPC calls

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use std::collections::{BTreeMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::str;
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info, trace};

use crate::entry::Entry;
use malachite_actors::gossip_mempool::{GossipMempoolRef, Msg as GossipMempoolMsg};
use malachite_actors::mempool::MempoolMsg;
use malachite_actors::mempool::MempoolRef;
use malachite_actors::util::forward;
use malachite_common::Transaction;
use malachite_gossip_mempool::{Event as GossipEvent, NetworkMsg, PeerId};
use malachite_node::config::{MempoolConfig, TestConfig};

#[allow(dead_code)]
pub struct KvMempool {
    gossip_mempool: GossipMempoolRef,
    mempool_config: MempoolConfig, // todo - pick only what's needed
    test_config: TestConfig,       // todo - pick only the mempool related
}

pub const OK: &[u8] = "ok\n".as_bytes();
pub const ERR_STR: &[u8] = "cannot parse into string\n".as_bytes();
pub const ERR_ENT: &[u8] = "cannot parse into Entry\n".as_bytes();

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

        // A basic HTTP server
        // Very ugly, needs refactoring
        tokio::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
            info!("basic HTTP server bootstrapped, listening");
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();

                let myself_inner = myself.clone();

                tokio::spawn(async move {
                    info!("got new connection ");
                    let mut buf = [0; 1024];

                    // In a loop, read data from the socket and write the data back.
                    loop {
                        let n = match socket.read(&mut buf).await {
                            // socket closed
                            Ok(n) if n == 0 => return,
                            Ok(n) => {
                                println!("yep got it {:#?}; sending a message to myself", n);
                                n
                            }
                            Err(e) => {
                                error!("failed to read from socket; err = {:?}", e);
                                return;
                            }
                        };

                        let reply = match str::from_utf8(&buf[0..n]) {
                            Ok(str) => {
                                if let Ok(_) = Entry::from_str(str) {
                                    let tx = Transaction::new(buf[0..n].to_vec());
                                    myself_inner.cast(MempoolMsg::Input(tx)).unwrap();
                                    OK
                                } else {
                                    ERR_ENT
                                }
                            }
                            Err(_) => ERR_STR,
                        };
                        if let Err(e) = socket.write_all(reply).await {
                            error!("failed to write to socket; err = {:?}", e);
                            return;
                        }
                    }
                });
            }
        });

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
                info!(
                    "KVMMPL: new transaction came in; total len={}; tx={:#?}",
                    state.transactions.len(),
                    tx
                );
                if state.transactions.len() < self.mempool_config.max_tx_count {
                    state.add_tx(&tx);
                } else {
                    info!("KVMMPL: Mempool is full, dropping transaction");
                }
            }

            // Adi: This is a request coming from the `Host` actor, specifically
            // from `build_new_proposal` via `run_build_proposal_task`
            MempoolMsg::TxStream { reply, .. } => {
                let txes = if state.transactions.len() > 0 {
                    let mut buffer = vec![];
                    while let Some((_, tx)) = state.transactions.pop_first() {
                        buffer.push(tx);

                        // TODO: This can be done more cleanly..
                        if buffer.len() >= 10 {
                            break;
                        }
                    }
                    info!(
                        "KVMMPL: Found transactions to reap len={}; left={}",
                        buffer.len(),
                        state.transactions.len()
                    );

                    buffer
                } else {
                    info!("No transaction to reap; returning empty vector");
                    vec![]
                };

                reply.send(txes)?;
            }

            MempoolMsg::Update { .. } => {
                // tx_hashes.iter().for_each(|hash| state.remove_tx(hash));

                // !!!
                // Adi important note: Disabling the behavior below otherwise
                // it will wipe out periodically the mempool.
                // FIXME: Reset the mempool for now
                // state.transactions.clear();
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

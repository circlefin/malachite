use std::time::Duration;

use async_trait::async_trait;
use ractor::{concurrency::JoinHandle, Actor, ActorProcessingErr, ActorRef};
use rand::rngs::SmallRng;
use rand::seq::IteratorRandom;
use rand::{Rng, RngCore, SeedableRng};
use tracing::debug;

use malachitebft_config::MempoolLoadType;
use malachitebft_starknet_p2p_types::{Transaction, Transactions};
use malachitebft_test_mempool::types::MempoolTransactionBatch;

use crate::proto::Protobuf;

use crate::{
    mempool::network::{MempoolNetworkMsg, MempoolNetworkRef},
    utils::ticker::ticker,
};

pub type MempoolLoadMsg = Msg;
pub type MempoolLoadRef = ActorRef<Msg>;

pub enum Msg {
    GenerateTransactions { count: usize, size: usize },
}

#[derive(Debug)]
pub struct State {
    ticker: JoinHandle<()>,
}

#[derive(Debug, Default)]
pub struct Params {
    pub load_type: MempoolLoadType,
}

pub struct MempoolLoad {
    params: Params,
    network: MempoolNetworkRef,
    span: tracing::Span,
}

impl MempoolLoad {
    pub fn new(params: Params, network: MempoolNetworkRef, span: tracing::Span) -> Self {
        Self {
            params,
            network,
            span,
        }
    }

    pub async fn spawn(
        params: Params,
        network: MempoolNetworkRef,
        span: tracing::Span,
    ) -> Result<MempoolLoadRef, ractor::SpawnErr> {
        debug!("spawning actor mempool_load");

        let actor = Self::new(params, network, span);
        let (actor_ref, _) = Actor::spawn(None, actor, ()).await?;
        Ok(actor_ref)
    }

    pub fn generate_transactions(count: usize, size: usize) -> Vec<Transaction> {
        let mut transactions: Vec<Transaction> = Vec::with_capacity(count);
        let mut rng = SmallRng::from_entropy();

        for _ in 0..count {
            let mut tx_bytes = vec![0; size];
            rng.fill_bytes(&mut tx_bytes);
            let tx = Transaction::new(tx_bytes);
            // debug!("transaction {:?}", tx.clone());

            transactions.push(tx);
        }
        debug!("MEMPOOL LOAD TX GENERATED {:?}", transactions.clone().len());

        transactions
    }
}

#[async_trait]
impl Actor for MempoolLoad {
    type Msg = Msg;
    type State = State;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: MempoolLoadRef,
        _args: (),
    ) -> Result<State, ActorProcessingErr> {
        debug!("starting ticker");

        let ticker = match self.params.load_type.clone() {
            MempoolLoadType::UniformLoad(uniform_load_config) => tokio::spawn(ticker(
                uniform_load_config.interval(),
                myself.clone(),
                move || Msg::GenerateTransactions {
                    count: uniform_load_config.count(),
                    size: uniform_load_config.size(),
                },
            )),
            MempoolLoadType::NoLoad => tokio::spawn(async {}),
            MempoolLoadType::NonUniformLoad(params) => tokio::spawn(async move {
                loop {
                    let mut rng = SmallRng::from_entropy();
                    // Determine if this iteration should generate a spike
                    let is_spike = rng.gen_bool(params.spike_probability());

                    // Vary transaction count and size
                    let count_variation = rng.gen_range(params.count_variation());
                    let size_variation = rng.gen_range(params.size_variation());

                    let count = if is_spike {
                        (params.base_count() - count_variation) as usize * params.spike_multiplier()
                    } else {
                        (params.base_count() + count_variation) as usize
                    };
                    let size = (params.base_size() + size_variation) as usize;

                    // Create and send the message
                    let msg = Msg::GenerateTransactions {
                        count: count.max(1), // Ensure count is at least 1
                        size: size.max(1),   // Ensure size is at least 1
                    };

                    if let Err(er) = myself.cast(msg) {
                        tracing::error!(?er, ?myself, "Failed to send tick message");
                        break;
                    }
                    // Random sleep between 100ms and 1s
                    let sleep_duration =
                        Duration::from_millis(params.sleep_interval().choose(&mut rng).unwrap());
                    debug!("sleeping thread for duration {:?}", sleep_duration);
                    tokio::time::sleep(sleep_duration).await;
                }
            }),
        };
        Ok(State { ticker })
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        state.ticker.abort();
        Ok(())
    }

    #[tracing::instrument("host.mempool_load", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        _myself: MempoolLoadRef,
        msg: Msg,
        _state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::GenerateTransactions { count, size } => {
                debug!("entered message handler GenerateTransactions");

                let transactions = Self::generate_transactions(count, size);
                debug!("broadcasting transactions {:?}", transactions.len());

                let tx_batch = Transactions::new(transactions).to_any().unwrap();
                debug!("broadcasting batch {:?}", tx_batch.clone().value.len());

                let mempool_batch: MempoolTransactionBatch = MempoolTransactionBatch::new(tx_batch);

                self.network
                    .cast(MempoolNetworkMsg::BroadcastMsg(mempool_batch))?;
                Ok(())
            }
        }
    }
}

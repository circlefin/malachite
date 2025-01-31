use std::time::Duration;

use async_trait::async_trait;
use rand::{Rng, RngCore};
use tracing::debug;
use ractor::{concurrency::JoinHandle, Actor, ActorProcessingErr, ActorRef};

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

#[derive(Debug)]
pub struct Params {
    pub load_type: MempoolLoadType,
}

pub struct MempoolLoad {
    params: Params,
    network: MempoolNetworkRef,
    span: tracing::Span,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            load_type: MempoolLoadType::NoLoad
        }
    }
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
        let mut rng = rand::thread_rng();

        for _ in 0..count {
            let mut tx_bytes = vec![0; size];
            rng.fill_bytes(&mut tx_bytes);
            let tx = Transaction::new(tx_bytes);
            // debug!("transaction {:?}", tx.clone());

            transactions.push(tx);
        }
        debug!("transactions generated {:?}", transactions.clone().len());

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

        let ticker = match self.params.load_type {
            MempoolLoadType::UniformLoad { count, size } => {
                debug!("entered uniform load branch");

                let interval = Duration::from_secs(1);
                tokio::spawn(ticker(interval, myself.clone(), move || {
                    Msg::GenerateTransactions { count, size }
                }))
            }
            MempoolLoadType::NoLoad => tokio::spawn(async {}),
            MempoolLoadType::NonUniformLoad => {
                debug!("entered nonuniform load branch");

                let mut rng = rand::thread_rng();
                let interval = Duration::from_secs(rng.gen_range(1..10));
                let count = rng.gen_range(500..=10000) as usize;
                let size = rng.gen_range(128..=512) as usize;
                tokio::spawn(ticker(interval, myself.clone(), move || {
                    Msg::GenerateTransactions { count, size }
                }))
            }
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

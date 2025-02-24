use std::path::PathBuf;

use async_trait::async_trait;
use malachitebft_config::{
    BootstrapProtocol, ConsensusConfig, RuntimeConfig, Selector, SyncConfig, TransportProtocol,
};
use rand::{CryptoRng, RngCore};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::task::JoinHandle;

use malachitebft_core_types::SigningProvider;
use malachitebft_engine::node::NodeRef;
use malachitebft_engine::util::events::RxEvent;

use crate::types::core::{Context, PrivateKey, PublicKey, VotingPower};
use crate::types::Keypair;

pub struct EngineHandle {
    pub actor: NodeRef,
    pub handle: JoinHandle<()>,
}

#[async_trait]
pub trait NodeHandle<Ctx>
where
    Self: Send + Sync + 'static,
    Ctx: Context,
{
    fn subscribe(&self) -> RxEvent<Ctx>;
    async fn kill(&self, reason: Option<String>) -> eyre::Result<()>;
}

pub trait NodeConfig {
    fn moniker(&self) -> &str;
    fn consensus(&self) -> &ConsensusConfig;
    fn sync(&self) -> &SyncConfig;
}

pub trait CanMakeConfig: Node {
    #[allow(clippy::too_many_arguments)]
    fn make_config(
        index: usize,
        total: usize,
        runtime: RuntimeConfig,
        enable_discovery: bool,
        bootstrap_protocol: BootstrapProtocol,
        selector: Selector,
        num_outbound_peers: usize,
        num_inbound_peers: usize,
        ephemeral_connection_timeout_ms: u64,
        transport: TransportProtocol,
    ) -> Self::Config;
}

pub trait CanGeneratePrivateKey: Node {
    fn generate_private_key<R>(&self, rng: R) -> PrivateKey<Self::Context>
    where
        R: RngCore + CryptoRng;
}

pub trait CanMakePrivateKeyFile: Node {
    fn make_private_key_file(&self, private_key: PrivateKey<Self::Context>)
        -> Self::PrivateKeyFile;
}

pub trait CanMakeGenesis: Node {
    fn make_genesis(
        &self,
        validators: Vec<(PublicKey<Self::Context>, VotingPower)>,
    ) -> Self::Genesis;
}

#[async_trait]
pub trait Node {
    type Context: Context;
    type Config: NodeConfig + Serialize + DeserializeOwned;
    type Genesis: Serialize + DeserializeOwned;
    type PrivateKeyFile: Serialize + DeserializeOwned;
    type SigningProvider: SigningProvider<Self::Context>;
    type NodeHandle: NodeHandle<Self::Context>;

    async fn start(&self) -> eyre::Result<Self::NodeHandle>;

    async fn run(self) -> eyre::Result<()>;

    fn get_home_dir(&self) -> PathBuf;

    fn load_config(&self) -> eyre::Result<Self::Config>;

    fn get_address(&self, pk: &PublicKey<Self::Context>) -> <Self::Context as Context>::Address;

    fn get_public_key(&self, pk: &PrivateKey<Self::Context>) -> PublicKey<Self::Context>;

    fn get_keypair(&self, pk: PrivateKey<Self::Context>) -> Keypair;

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey<Self::Context>;

    fn load_private_key_file(&self) -> eyre::Result<Self::PrivateKeyFile>;

    fn get_signing_provider(&self, private_key: PrivateKey<Self::Context>)
        -> Self::SigningProvider;

    fn load_genesis(&self) -> eyre::Result<Self::Genesis>;
}

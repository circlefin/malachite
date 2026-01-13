#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;

use async_trait::async_trait;
use malachitebft_app::config::NodeConfig;
use malachitebft_app::events::RxEvent;
use serde::de::DeserializeOwned;
use serde::Serialize;

use malachitebft_signing::SigningProvider;

use malachitebft_core_types::{Context, PrivateKey, PublicKey};

pub use libp2p_identity::Keypair;

#[async_trait]
pub trait NodeHandle<Ctx>
where
    Self: Send + Sync + 'static,
    Ctx: Context,
{
    fn subscribe(&self) -> RxEvent<Ctx>;
    async fn kill(&self, reason: Option<String>) -> eyre::Result<()>;
}

#[async_trait]
pub trait Node {
    type Context: Context;
    type Config: NodeConfig + Serialize + DeserializeOwned;
    type Genesis: Serialize + DeserializeOwned;
    type PrivateKeyFile: Serialize + DeserializeOwned;
    type SigningProvider: SigningProvider<Self::Context> + 'static;
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

    fn load_genesis(&self) -> eyre::Result<Self::Genesis>;

    fn get_signing_provider(&self, private_key: PrivateKey<Self::Context>)
        -> Self::SigningProvider;
}

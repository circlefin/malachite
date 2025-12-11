use rand::{CryptoRng, RngCore};

use malachitebft_config::{DiscoveryConfig, RuntimeConfig, TransportProtocol, ValueSyncConfig};
use malachitebft_core_types::{PrivateKey, PublicKey, VotingPower};

use crate::node::Node;

#[derive(Copy, Clone, Debug)]
pub struct MakeConfigSettings {
    pub runtime: RuntimeConfig,
    pub transport: TransportProtocol,
    pub discovery: DiscoveryConfig,
    pub value_sync: ValueSyncConfig,
    pub persistent_peers_only: bool,
}

pub trait CanMakeConfig: Node {
    fn make_config(index: usize, total: usize, settings: MakeConfigSettings) -> Self::Config;
}

pub trait CanMakeDistributedConfig: Node {
    fn make_distributed_config(
        index: usize,
        total: usize,
        machines: Vec<String>,
        bootstrap_set_size: usize,
        settings: MakeConfigSettings,
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

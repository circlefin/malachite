use std::path::PathBuf;

use ractor::async_trait;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use malachitebft_app::events::{RxEvent, TxEvent};
use malachitebft_app::types::Keypair;
use malachitebft_app::{Node, NodeHandle};
use malachitebft_config::Config;
use malachitebft_core_types::VotingPower;
use malachitebft_engine::node::NodeRef;
use malachitebft_starknet_p2p_types::Ed25519Provider;

use crate::spawn::spawn_node_actor;
use crate::types::{Address, Height, MockContext, PrivateKey, PublicKey, Validator, ValidatorSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genesis {
    pub validator_set: ValidatorSet,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateKeyFile {
    pub private_key: PrivateKey,
    pub public_key: PublicKey,
    pub address: Address,
}

impl From<PrivateKey> for PrivateKeyFile {
    fn from(private_key: PrivateKey) -> Self {
        let public_key = private_key.public_key();
        let address = Address::from_public_key(public_key);

        Self {
            private_key,
            public_key,
            address,
        }
    }
}

pub struct Handle {
    pub actor: NodeRef,
    pub handle: JoinHandle<()>,
    pub tx_event: TxEvent<MockContext>,
}

#[async_trait]
impl NodeHandle<MockContext> for Handle {
    fn subscribe(&self) -> RxEvent<MockContext> {
        self.tx_event.subscribe()
    }

    async fn kill(&self, _reason: Option<String>) -> eyre::Result<()> {
        self.actor.kill_and_wait(None).await?;
        self.handle.abort();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct StarknetNode {
    pub config: Config,
    pub home_dir: PathBuf,
    pub start_height: Option<u64>,
}

impl StarknetNode {
    pub fn genesis_file(&self) -> PathBuf {
        self.home_dir.join("config").join("genesis.json")
    }

    pub fn private_key_file(&self) -> PathBuf {
        self.home_dir.join("config").join("priv_validator_key.json")
    }
}

#[async_trait]
impl Node for StarknetNode {
    type Context = MockContext;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKeyFile;
    type SigningProvider = Ed25519Provider;
    type NodeHandle = Handle;

    fn get_home_dir(&self) -> PathBuf {
        self.home_dir.to_owned()
    }

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }

    fn get_address(&self, pk: &PublicKey) -> Address {
        Address::from_public_key(*pk)
    }

    fn get_public_key(&self, pk: &PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn get_keypair(&self, pk: PrivateKey) -> Keypair {
        Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file.private_key
    }

    fn load_private_key_file(&self) -> std::io::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(self.private_key_file())?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        PrivateKeyFile::from(private_key)
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Self::SigningProvider::new(private_key)
    }

    fn load_genesis(&self) -> std::io::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(self.genesis_file())?;
        serde_json::from_str(&genesis).map_err(|e| e.into())
    }

    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }

    async fn start(&self) -> eyre::Result<Handle> {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _enter = span.enter();

        let priv_key_file = self.load_private_key_file()?;
        let private_key = self.load_private_key(priv_key_file);
        let genesis = self.load_genesis()?;
        let tx_event = TxEvent::new();

        let start_height = self.start_height.map(|height| Height::new(height, 1));

        let (actor, handle) = spawn_node_actor(
            self.config.clone(),
            self.home_dir.clone(),
            genesis.validator_set,
            private_key,
            start_height,
            tx_event.clone(),
            span.clone(),
        )
        .await;

        Ok(Handle {
            actor,
            handle,
            tx_event,
        })
    }

    async fn run(self) -> eyre::Result<()> {
        let handle = self.start().await?;
        handle.actor.wait(None).await.map_err(Into::into)
    }
}

#[test]
fn test_starknet_node() {
    // Create temp folder for configuration files
    let temp_dir = tempfile::TempDir::with_prefix("informalsystems-malachitebft-node-")
        .expect("Failed to create temp dir");

    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    std::fs::create_dir_all(temp_path.join("config")).unwrap();

    // Create default configuration
    let node = StarknetNode {
        home_dir: temp_path.clone(),
        config: Config {
            moniker: "test-node".to_string(),
            ..Default::default()
        },
        start_height: Some(1),
    };

    // Create configuration files
    use malachitebft_test_cli::*;

    let priv_keys = new::generate_private_keys(&node, 1, true);
    let pub_keys = priv_keys.iter().map(|pk| node.get_public_key(pk)).collect();
    let genesis = new::generate_genesis(&node, pub_keys, true);

    file::save_priv_validator_key(
        &node,
        &node.private_key_file(),
        &PrivateKeyFile::from(priv_keys[0].clone()),
    )
    .unwrap();

    file::save_genesis(&node, &node.genesis_file(), &genesis).unwrap();

    // Run the node for a few seconds
    const TIMEOUT: u64 = 3;
    use tokio::time::{timeout, Duration};
    let rt = malachitebft_test_cli::runtime::build_runtime(node.config.runtime).unwrap();
    let result = rt.block_on(async { timeout(Duration::from_secs(TIMEOUT), node.run()).await });

    // Check that the node did not quit before the timeout.
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "deadline has elapsed");
    let io_error: std::io::Error = error.into();
    assert_eq!(
        io_error.to_string(),
        std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out").to_string()
    );
}

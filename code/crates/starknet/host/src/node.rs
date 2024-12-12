use std::path::{Path, PathBuf};

use libp2p_identity::ecdsa;
use ractor::async_trait;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use tracing::{info, Instrument};

use malachite_actors::util::events::TxEvent;
use malachite_app::types::Keypair;
use malachite_app::Node;
use malachite_config::Config;
use malachite_core_types::VotingPower;

use crate::spawn::spawn_node_actor;
use crate::types::Height;
use crate::types::MockContext;
use crate::types::{Address, PrivateKey, PublicKey, Validator, ValidatorSet};

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

pub struct StarknetNode {
    pub config: Config,
    pub home_dir: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<u64>,
}

#[async_trait]
impl Node for StarknetNode {
    type Context = MockContext;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKeyFile;

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
        let pk_bytes = pk.inner().to_bytes_be();
        let secret_key = ecdsa::SecretKey::try_from_bytes(pk_bytes).unwrap();
        let ecdsa_keypair = ecdsa::Keypair::from(secret_key);
        Keypair::from(ecdsa_keypair)
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file.private_key
    }

    fn load_private_key_file(
        &self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(path)?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        PrivateKeyFile::from(private_key)
    }

    fn load_genesis(&self, path: impl AsRef<Path>) -> std::io::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(path)?;
        serde_json::from_str(&genesis).map_err(|e| e.into())
    }

    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }

    async fn run(&self) -> eyre::Result<()> {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _enter = span.enter();

        let priv_key_file = self.load_private_key_file(self.private_key_file.clone())?;

        let private_key = self.load_private_key(priv_key_file);

        let genesis = self.load_genesis(self.genesis_file.clone())?;

        let start_height = self.start_height.map(|height| Height::new(height, 1));

        let (actor, handle) = spawn_node_actor(
            self.config.clone(),
            self.home_dir.clone(),
            genesis.validator_set,
            private_key,
            start_height,
            TxEvent::new(),
            span.clone(),
        )
        .await;

        tokio::spawn({
            let actor = actor.clone();
            {
                async move {
                    tokio::signal::ctrl_c().await.unwrap();
                    info!("Shutting down...");
                    actor.stop(None);
                }
            }
            .instrument(span.clone())
        });

        handle.await?;

        Ok(())
    }
}

#[test]
fn test_starknet_node() {
    // Create temp folder for configuration files
    let temp_dir =
        tempfile::TempDir::with_prefix("malachite-node-").expect("Failed to create temp dir");

    let temp_path = temp_dir.path().to_owned();

    if std::env::var("KEEP_TEMP").is_ok() {
        std::mem::forget(temp_dir);
    }

    // Create default configuration
    let node = StarknetNode {
        home_dir: temp_path.clone(),
        config: Config {
            moniker: "test-node".to_string(),
            ..Default::default()
        },
        genesis_file: temp_path.join("genesis.json"),
        private_key_file: temp_path.join("private_key.json"),
        start_height: Some(1),
    };

    // Create configuration files
    use malachite_cli::*;

    let priv_keys = new::generate_private_keys(&node, 1, true);
    let pub_keys = priv_keys.iter().map(|pk| node.get_public_key(pk)).collect();
    let genesis = new::generate_genesis(&node, pub_keys, true);

    file::save_priv_validator_key(
        &node,
        &node.private_key_file,
        &PrivateKeyFile::from(priv_keys[0]),
    )
    .unwrap();

    file::save_genesis(&node, &node.genesis_file, &genesis).unwrap();

    // Run the node for a few seconds
    const TIMEOUT: u64 = 3;
    use tokio::time::{timeout, Duration};
    let rt = malachite_cli::runtime::build_runtime(node.config.runtime).unwrap();
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

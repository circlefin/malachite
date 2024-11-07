use std::path::{Path, PathBuf};

use rand::{CryptoRng, RngCore};
use tracing::{info, Instrument};

use crate::mock::context::MockContext;
use crate::types::{PrivateKey, PublicKey, Validator, ValidatorSet};
use malachite_common::VotingPower;
use malachite_config::Config;
use malachite_node::Node;

use crate::spawn::spawn_node_actor;
use crate::types::Height;

pub struct StarknetNode {
    pub config: Config,
    pub home_dir: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<u64>,
}

impl Node for StarknetNode {
    type Context = MockContext;
    type PrivateKeyFile = PrivateKey;
    type Genesis = ValidatorSet;

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey
    where
        R: RngCore + CryptoRng,
    {
        PrivateKey::generate(rng)
    }

    fn generate_public_key(&self, pk: PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn load_private_key_file(
        &self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(path)?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file
    }

    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }

    fn load_genesis(&self, path: impl AsRef<Path>) -> std::io::Result<Self::Genesis> {
        let genesis = std::fs::read_to_string(path)?;
        serde_json::from_str(&genesis).map_err(|e| e.into())
    }

    fn make_genesis(&self, validators: Vec<(PublicKey, VotingPower)>) -> Self::Genesis {
        let validators = validators
            .into_iter()
            .map(|(pk, vp)| Validator::new(pk, vp));

        ValidatorSet::new(validators)
    }

    async fn run(&self) {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _enter = span.enter();

        let priv_key_file = self
            .load_private_key_file(self.private_key_file.clone())
            .unwrap();

        let private_key = self.load_private_key(priv_key_file);

        let genesis = self.load_genesis(self.genesis_file.clone()).unwrap();

        let start_height = self.start_height.map(|height| Height::new(height, 1));

        let (actor, handle) = spawn_node_actor(
            self.config.clone(),
            self.home_dir.clone(),
            genesis,
            private_key,
            start_height,
            None,
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

        handle.await.unwrap();
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
    let pub_keys = priv_keys
        .iter()
        .map(|pk| node.generate_public_key(*pk))
        .collect();
    let genesis = new::generate_genesis(&node, pub_keys, true);
    file::save_priv_validator_key(&node, &node.private_key_file, &priv_keys[0]).unwrap();
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

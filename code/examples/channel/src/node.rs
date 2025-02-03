//! The Application (or Node) definition. The Node trait implements the Consensus context and the
//! cryptographic library used for signing.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rand::{CryptoRng, RngCore};

use malachitebft_app_channel::app::metrics::SharedRegistry;
use malachitebft_app_channel::app::types::config::Config;
use malachitebft_app_channel::app::types::core::VotingPower;
use malachitebft_app_channel::app::types::Keypair;
use malachitebft_app_channel::app::Node;

// Use the same types used for integration tests.
// A real application would use its own types and context instead.
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, PrivateKey, PublicKey, TestContext, Validator,
    ValidatorSet,
};
use malachitebft_test_cli::metrics;

use crate::metrics::DbMetrics;
use crate::state::State;
use crate::store::Store;

/// Main application struct implementing the consensus node functionality
#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub home_dir: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<Height>,
}

#[async_trait]
impl Node for App {
    type Context = TestContext;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;
    type SigningProvider = Ed25519Provider;

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
        Address::from_public_key(pk)
    }

    fn get_public_key(&self, pk: &PrivateKey) -> PublicKey {
        pk.public_key()
    }

    fn get_keypair(&self, pk: PrivateKey) -> Keypair {
        Keypair::ed25519_from_bytes(pk.inner().to_bytes()).unwrap()
    }

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey {
        file
    }

    fn load_private_key_file(
        &self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Self::PrivateKeyFile> {
        let private_key = std::fs::read_to_string(path)?;
        serde_json::from_str(&private_key).map_err(|e| e.into())
    }

    fn make_private_key_file(&self, private_key: PrivateKey) -> Self::PrivateKeyFile {
        private_key
    }

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Ed25519Provider::new(private_key)
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

    async fn run(self) -> eyre::Result<()> {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _enter = span.enter();

        let private_key_file = self.load_private_key_file(&self.private_key_file)?;
        let private_key = self.load_private_key(private_key_file);
        let public_key = self.get_public_key(&private_key);
        let address = self.get_address(&public_key);
        let signing_provider = self.get_signing_provider(private_key);
        let ctx = TestContext::new();

        let genesis = self.load_genesis(self.genesis_file.clone())?;
        let initial_validator_set = genesis.validator_set.clone();

        let codec = ProtobufCodec;

        let mut channels = malachitebft_app_channel::run(
            ctx,
            codec,
            self.clone(),
            self.config.clone(),
            self.private_key_file.clone(),
            self.start_height,
            initial_validator_set,
        )
        .await?;

        let registry = SharedRegistry::global().with_moniker(&self.config.moniker);
        let metrics = DbMetrics::register(&registry);

        if self.config.metrics.enabled {
            tokio::spawn(metrics::serve(self.config.metrics.listen_addr));
        }

        let store = Store::open(self.get_home_dir().join("store.db"), metrics)?;
        let start_height = self.start_height.unwrap_or_default();
        let mut state = State::new(ctx, signing_provider, genesis, address, start_height, store);

        crate::app::run(&mut state, &mut channels).await
    }
}

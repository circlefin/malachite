//! The Application (or Node) definition. The Node trait implements the Consensus context and the
//! cryptographic library used for signing.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rand::{CryptoRng, RngCore};
use tokio::task::JoinHandle;
use tracing::Instrument;

use malachitebft_app_channel::app::events::TxEvent;
use malachitebft_app_channel::app::types::config::Config; // TODO: Move into test app
use malachitebft_app_channel::app::types::core::VotingPower;
use malachitebft_app_channel::app::types::Keypair;
use malachitebft_app_channel::app::Node;
use malachitebft_app_channel::EngineHandle;

// Use the same types used for integration tests.
// A real application would use its own types and context instead.
use malachitebft_test::codec::proto::ProtobufCodec;
use malachitebft_test::{
    Address, Ed25519Provider, Genesis, Height, PrivateKey, PublicKey, TestContext, Validator,
    ValidatorSet,
};

use crate::state::State;
use crate::store::Store;

pub struct Handles {
    pub app: JoinHandle<()>,
    pub engine: EngineHandle,
    pub tx_event: TxEvent<TestContext>,
}

/// Main application struct implementing the consensus node functionality
#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub home_dir: PathBuf,
    pub validator_set: ValidatorSet,
    pub private_key: PrivateKey,
    pub start_height: Option<Height>,
}

impl App {
    pub async fn start(&self) -> eyre::Result<Handles> {
        let span = tracing::error_span!("node", moniker = %self.config.moniker);
        let _guard = span.enter();

        let public_key = self.get_public_key(&self.private_key);
        let address = self.get_address(&public_key);
        let private_key_file = self.load_private_key_file()?;
        let private_key = self.load_private_key(private_key_file);
        let signing_provider = self.get_signing_provider(private_key);

        let validators = self
            .validator_set
            .validators
            .iter()
            .map(|v| (v.public_key, v.voting_power))
            .collect();

        let genesis = self.make_genesis(validators);

        let ctx = TestContext::new();
        let codec = ProtobufCodec;

        let (mut channels, engine_handle) = malachitebft_app_channel::start_engine(
            ctx,
            codec,
            self.clone(),
            self.config.clone(),
            self.start_height,
            self.validator_set.clone(),
        )
        .await?;

        drop(_guard);

        let config = self.config.clone();
        let store = Store::open(self.get_home_dir().join("store.db"))?;
        let start_height = self.start_height.unwrap_or_default();

        let mut state = State::new(
            ctx,
            config,
            genesis.clone(),
            address,
            start_height,
            store,
            signing_provider,
        );

        let tx_event = channels.events.clone();

        let app_handle = tokio::spawn(
            async move {
                if let Err(e) = crate::app::run(genesis, &mut state, &mut channels).await {
                    tracing::error!("Application has failed with an error: {e}");
                }
            }
            .instrument(span),
        );

        Ok(Handles {
            app: app_handle,
            engine: engine_handle,
            tx_event,
        })
    }
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

    fn get_signing_provider(&self, private_key: PrivateKey) -> Self::SigningProvider {
        Ed25519Provider::new(private_key)
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

    fn load_private_key_file(&self) -> std::io::Result<Self::PrivateKeyFile> {
        Ok(self.private_key.clone())
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

        let validator_set = ValidatorSet::new(validators);

        Genesis { validator_set }
    }

    async fn run(self) -> eyre::Result<()> {
        let handles = self.start().await?;
        handles.app.await.map_err(Into::into)
    }
}

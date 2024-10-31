use std::path::{Path, PathBuf};

use crate::spawn::spawn_node_actor;
use malachite_common::VotingPower;
use malachite_config::Config;
use malachite_node::Node;
use malachite_starknet_host::mock::context::MockContext;
use malachite_starknet_host::types::{PrivateKey, PublicKey, Validator, ValidatorSet};
use rand::{CryptoRng, RngCore};
use tracing::{info, Instrument};

pub struct StarknetNode {
    pub config: Config,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
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
        let span = tracing::error_span!("node", moniker=%self.config.clone().moniker);
        let _enter = span.enter();

        let priv_key_file = self
            .load_private_key_file(self.private_key_file.clone())
            .unwrap();
        let private_key = self.load_private_key(priv_key_file);
        let genesis = self.load_genesis(self.genesis_file.clone()).unwrap();
        let (actor, handle) =
            spawn_node_actor(self.config.clone(), genesis, private_key, None).await;

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

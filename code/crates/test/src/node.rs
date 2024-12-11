use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rand::{CryptoRng, RngCore};

use malachite_app::types::Keypair;
use malachite_app::Node;
use malachite_common::VotingPower;
use malachite_config::Config;

use crate::context::TestContext;
use crate::{Address, Genesis, PrivateKey, PublicKey, Validator, ValidatorSet};

pub struct TestNode {
    pub config: Config,
    pub home_dir: PathBuf,
    pub genesis_file: PathBuf,
    pub private_key_file: PathBuf,
    pub start_height: Option<u64>,
}

#[async_trait]
impl Node for TestNode {
    type Context = TestContext;
    type Genesis = Genesis;
    type PrivateKeyFile = PrivateKey;

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
        unimplemented!()
    }
}

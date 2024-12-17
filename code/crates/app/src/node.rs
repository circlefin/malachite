use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rand::{CryptoRng, RngCore};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::types::core::{Context, PrivateKey, PublicKey, VotingPower};
use crate::types::Keypair;

#[async_trait]
pub trait Node {
    type Context: Context;
    type Genesis: Serialize + DeserializeOwned;
    type PrivateKeyFile: Serialize + DeserializeOwned;

    fn get_home_dir(&self) -> PathBuf;

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey<Self::Context>
    where
        R: RngCore + CryptoRng;

    fn get_address(&self, pk: &PublicKey<Self::Context>) -> <Self::Context as Context>::Address;

    fn get_public_key(&self, pk: &PrivateKey<Self::Context>) -> PublicKey<Self::Context>;

    fn get_keypair(&self, pk: PrivateKey<Self::Context>) -> Keypair;

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey<Self::Context>;

    fn load_private_key_file(&self, path: impl AsRef<Path>) -> io::Result<Self::PrivateKeyFile>;

    fn make_private_key_file(&self, private_key: PrivateKey<Self::Context>)
        -> Self::PrivateKeyFile;

    fn load_genesis(&self, path: impl AsRef<Path>) -> io::Result<Self::Genesis>;

    fn make_genesis(
        &self,
        validators: Vec<(PublicKey<Self::Context>, VotingPower)>,
    ) -> Self::Genesis;

    async fn run(self) -> eyre::Result<()>;
}

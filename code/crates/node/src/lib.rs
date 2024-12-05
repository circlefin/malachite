// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::future::Future;
use std::path::{Path, PathBuf};

use rand::{CryptoRng, RngCore};
use serde::de::DeserializeOwned;
use serde::Serialize;

use malachite_common::{Context, PrivateKey, PublicKey, VotingPower};

pub trait Node {
    type Context: Context;
    type Genesis: Serialize + DeserializeOwned;
    type PrivateKeyFile: Serialize + DeserializeOwned;

    fn get_home_dir(&self) -> PathBuf;

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey<Self::Context>
    where
        R: RngCore + CryptoRng;

    fn generate_public_key(&self, pk: PrivateKey<Self::Context>) -> PublicKey<Self::Context>;

    fn load_private_key(&self, file: Self::PrivateKeyFile) -> PrivateKey<Self::Context>;

    fn load_private_key_file(
        &self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Self::PrivateKeyFile>;

    fn make_private_key_file(&self, private_key: PrivateKey<Self::Context>)
        -> Self::PrivateKeyFile;

    fn load_genesis(&self, path: impl AsRef<Path>) -> std::io::Result<Self::Genesis>;

    fn make_genesis(
        &self,
        validators: Vec<(PublicKey<Self::Context>, VotingPower)>,
    ) -> Self::Genesis;

    fn run(&self) -> impl Future<Output = ()> + Send;
}

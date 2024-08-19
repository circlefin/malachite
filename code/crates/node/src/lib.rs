// For coverage on nightly
#![allow(unexpected_cfgs)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::path::Path;

use malachite_common::{Context, PrivateKey, PublicKey, VotingPower};
use rand::{CryptoRng, RngCore};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub mod config;

pub trait Node {
    type Context: Context;
    type Genesis: Serialize + DeserializeOwned;
    type PrivateKeyFile: Serialize + DeserializeOwned;

    fn generate_private_key<R>(&self, rng: R) -> PrivateKey<Self::Context>
    where
        R: RngCore + CryptoRng;

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
}

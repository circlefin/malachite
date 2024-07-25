use std::path::PathBuf;

use color_eyre::eyre::Result;
use tracing::info;

use malachite_starknet_host::types::{Address, PrivateKey};

use crate::args::Args;

#[derive(clap::Args, Clone, Debug)]
pub struct GenerateCmd {
    #[clap(short, long, value_name = "OUTPUT_FILE")]
    output: PathBuf,
}

impl GenerateCmd {
    pub fn run(&self, _args: &Args) -> Result<()> {
        let rng = rand::thread_rng();
        let pk = PrivateKey::generate(rng);

        let address = Address::from_public_key(pk.public_key());
        info!("Generated key with address: {address}");

        let public_key = pk.public_key();
        info!("Public key: {}", serde_json::to_string_pretty(&public_key)?);

        info!("Saving private key to {:?}", self.output);
        std::fs::write(&self.output, serde_json::to_vec(&pk)?)?;

        Ok(())
    }
}

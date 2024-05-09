/// Init command
use std::fs::File;
use std::path::PathBuf;

use malachite_node::config::Config;
use malachite_test::PrivateKey;

use crate::example::{generate_config, generate_genesis, generate_private_key};
use tracing::debug;

use malachite_test::ValidatorSet as Genesis;

/// Execute the init command
pub fn run(
    config_file: PathBuf,
    genesis_file: PathBuf,
    priv_validator_key_file: PathBuf,
    index: usize,
) {
    // Save default configuration
    if !config_file.exists() {
        debug!("Saving configuration to {:?}.", config_file);
        save_config(&config_file, &generate_config(index));
    }
    // Save default genesis
    if !genesis_file.exists() {
        debug!("Saving test genesis to {:?}.", genesis_file);
        save_genesis(&genesis_file, &generate_genesis());
    }
    // Save default priv_validator_key
    if !priv_validator_key_file.exists() {
        debug!("Saving private key to {:?}.", priv_validator_key_file);
        save_priv_validator_key(&priv_validator_key_file, &generate_private_key(index));
    }
}

/// Save configuration to file
pub fn save_config(config_file: &PathBuf, cfg: &Config) {
    confy::store_path(config_file, cfg).unwrap();
}

/// Save genesis to file
pub fn save_genesis(genesis_file: &PathBuf, genesis: &Genesis) {
    let file = File::create(genesis_file).unwrap();
    serde_json::to_writer_pretty(file, genesis).unwrap();
}

/// Save private_key validator key to file
pub fn save_priv_validator_key(priv_validator_key_file: &PathBuf, private_key: &PrivateKey) {
    let file = File::create(priv_validator_key_file).unwrap();
    serde_json::to_writer_pretty(file, private_key).unwrap();
}

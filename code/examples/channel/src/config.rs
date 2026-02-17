#![allow(unused_imports)]

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use malachitebft_app_channel::app::config::{
    ConsensusConfig, LogFormat, LogLevel, LoggingConfig, MetricsConfig, NodeConfig, RuntimeConfig,
    ValueSyncConfig,
};

/// Configuration for validator set rotation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidatorRotationConfig {
    /// Whether to enable validator rotation. Default: false (no rotation)
    #[serde(default)]
    pub enabled: bool,

    /// Rotate the validator set every N blocks. Default: 10
    /// Only used when `enabled` is true.
    #[serde(default = "default_rotation_period")]
    pub rotation_period: u64,

    /// Number of validators to select from the full set. Default: 0 (use all)
    /// If 0 or >= total validators, uses all validators.
    /// Only used when `enabled` is true.
    #[serde(default)]
    pub selection_size: usize,
}

fn default_rotation_period() -> u64 {
    10
}

impl Default for ValidatorRotationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rotation_period: default_rotation_period(),
            selection_size: 0,
        }
    }
}

/// Malachite configuration options
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// A custom human-readable name for this node
    pub moniker: String,

    /// Log configuration options
    pub logging: LoggingConfig,

    /// Consensus configuration options
    pub consensus: ConsensusConfig,

    /// ValueSync configuration options
    pub value_sync: ValueSyncConfig,

    /// Metrics configuration options
    pub metrics: MetricsConfig,

    /// Runtime configuration options
    pub runtime: RuntimeConfig,

    /// Validator rotation configuration options
    #[serde(default)]
    pub validator_rotation: ValidatorRotationConfig,
}

impl NodeConfig for Config {
    fn moniker(&self) -> &str {
        &self.moniker
    }

    fn consensus(&self) -> &ConsensusConfig {
        &self.consensus
    }

    fn consensus_mut(&mut self) -> &mut ConsensusConfig {
        &mut self.consensus
    }

    fn value_sync(&self) -> &ValueSyncConfig {
        &self.value_sync
    }

    fn value_sync_mut(&mut self) -> &mut ValueSyncConfig {
        &mut self.value_sync
    }
}

/// load_config parses the environment variables and loads the provided config file path
/// to create a Config struct.
pub fn load_config(path: impl AsRef<Path>, prefix: Option<&str>) -> eyre::Result<Config> {
    ::config::Config::builder()
        .add_source(::config::File::from(path.as_ref()))
        .add_source(
            ::config::Environment::with_prefix(prefix.unwrap_or("MALACHITE")).separator("__"),
        )
        .build()?
        .try_deserialize()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config_file() {
        let file = include_str!("../config.toml");
        let config = toml::from_str::<Config>(file).unwrap();
        assert_eq!(config.consensus.queue_capacity, 0);

        let tmp_file = std::env::temp_dir().join("config-test.toml");
        std::fs::write(&tmp_file, file).unwrap();

        let config = load_config(&tmp_file, None).unwrap();
        assert_eq!(config.consensus.queue_capacity, 0);

        std::fs::remove_file(tmp_file).unwrap();
    }
}

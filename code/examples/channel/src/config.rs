#![allow(unused_imports)]

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use malachitebft_app_channel::app::config::{
    ConsensusConfig, LogFormat, LogLevel, LoggingConfig, MetricsConfig, RuntimeConfig,
    TimeoutConfig, ValueSyncConfig,
};

use malachitebft_app_channel::app::node::NodeConfig;

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
        assert_eq!(config.consensus.timeouts, TimeoutConfig::default());

        let tmp_file = std::env::temp_dir().join("config-test.toml");
        std::fs::write(&tmp_file, file).unwrap();

        let config = load_config(&tmp_file, None).unwrap();
        assert_eq!(config.consensus.timeouts, TimeoutConfig::default());

        std::fs::remove_file(tmp_file).unwrap();
    }
}

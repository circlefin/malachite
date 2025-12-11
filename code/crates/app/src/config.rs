pub use malachitebft_config::*;

pub trait NodeConfig {
    fn moniker(&self) -> &str;

    fn consensus(&self) -> &ConsensusConfig;
    fn consensus_mut(&mut self) -> &mut ConsensusConfig;

    fn value_sync(&self) -> &ValueSyncConfig;
    fn value_sync_mut(&mut self) -> &mut ValueSyncConfig;
}

use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre;
use malachitebft_core_types::Context;
use tracing::{error, info};

use malachitebft_app::engine::wal::{log_entries, WalCodec};
use malachitebft_app::wal::Log;

#[derive(Parser, Debug, Clone, Default, PartialEq)]
pub struct DumpWalCmd {
    pub wal_file: PathBuf,
}

impl DumpWalCmd {
    pub fn run<Ctx, Codec>(&self, codec: Codec) -> eyre::Result<()>
    where
        Ctx: Context,
        Codec: WalCodec<Ctx>,
    {
        let mut log = Log::open(&self.wal_file)?;

        let len = log.len();
        let mut count = 0;

        info!("WAL Dump");
        info!("- Entries: {len}");
        info!("- Size:    {} bytes", log.size_bytes().unwrap_or(0));
        info!("Entries:");

        for (idx, entry) in log_entries(&mut log, &codec)?.enumerate() {
            count += 1;

            match entry {
                Ok(entry) => {
                    info!("- #{idx}: {entry:?}");
                }
                Err(e) => {
                    error!("- #{idx}: Error decoding WAL entry: {e}");
                }
            }
        }

        if count != len {
            error!("Expected {len} entries, but found {count} entries");
        }

        Ok(())
    }
}

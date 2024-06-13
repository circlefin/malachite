use clap::Subcommand;
use color_eyre::eyre::Result;

use crate::args::Args;

pub mod generate;

/// Manage keys
#[derive(Subcommand, Clone, Debug)]
pub enum KeysCmd {
    /// Generate a new key
    Generate(generate::GenerateCmd),
}

impl KeysCmd {
    pub fn run(&self, args: &Args) -> Result<()> {
        match self {
            KeysCmd::Generate(cmd) => cmd.run(args),
        }
    }
}

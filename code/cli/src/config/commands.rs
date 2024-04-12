use clap::Subcommand;

#[derive(Subcommand, Clone, Debug, Default)]
pub enum Commands {
    /// Initialize configuration
    Init,
    /// Start node
    #[default]
    Start,
}

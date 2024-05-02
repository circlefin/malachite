use core::fmt;

use clap::ValueEnum;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::FmtSubscriber;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum DebugSection {
    Ractor,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

pub fn init(log_level: LogLevel, debug_sections: &[DebugSection]) {
    color_eyre::install().expect("Failed to install global error handler");

    let filter = build_tracing_filter(log_level, debug_sections);

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    let subscriber = builder.finish();
    subscriber.init();
}

/// Check if both stdout and stderr are proper terminal (tty),
/// so that we know whether or not to enable colored output,
/// using ANSI escape codes. If either is not, eg. because
/// stdout is redirected to a file, we don't enable colored output.
pub fn enable_ansi() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
}

/// The relayer crates targeted by the default log level.
const TARGET_CRATES: &[&str] = &["malachite"];

/// Build a tracing directive setting the log level for the relayer crates to the
/// given `log_level`.
pub fn default_directive(log_level: LogLevel) -> String {
    use itertools::Itertools;

    TARGET_CRATES
        .iter()
        .map(|&c| format!("{c}={log_level}"))
        .join(",")
}

/// Builds a tracing filter based on the input `log_level`.
/// Enables tracing exclusively for the relayer crates.
/// Returns error if the filter failed to build.
fn build_tracing_filter(default_level: LogLevel, debug_sections: &[DebugSection]) -> EnvFilter {
    let mut directive =
        std::env::var("RUST_LOG").unwrap_or_else(|_| default_directive(default_level));

    if debug_sections.contains(&DebugSection::Ractor) {
        // Enable debug tracing for the `ractor` crate as well
        directive.push_str(",ractor=debug");
    }

    // Build the filter directive
    match EnvFilter::try_new(&directive) {
        Ok(out) => out,
        Err(e) => panic!(
            "ERROR: unable to initialize Malachite with log filtering directive {directive:?}: {e}"
        ),
    }
}

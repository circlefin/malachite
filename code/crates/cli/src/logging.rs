use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::FmtSubscriber;

use malachite_node::config::{LogFormat, LogLevel};

pub fn init(log_level: LogLevel, log_format: LogFormat) {
    let log_level = if let Ok(rust_log) = std::env::var("RUST_LOG") {
        rust_log
    } else {
        log_level.to_string()
    };

    let filter = build_tracing_filter(&log_level);

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    // There must be a better way to use conditionals in the builder pattern.
    match log_format {
        LogFormat::Plaintext => {
            let subscriber = builder.finish();
            subscriber.init();
        }
        LogFormat::Json => {
            let subscriber = builder.json().finish();
            subscriber.init();
        }
    };
}

/// Check if both stdout and stderr are proper terminal (tty),
/// so that we know whether or not to enable colored output,
/// using ANSI escape codes. If either is not, eg. because
/// stdout is redirected to a file, we don't enable colored output.
pub fn enable_ansi() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
}

/// Common prefixes of the crates targeted by the default log level.
const TARGET_CRATES: &[&str] = &["malachite"];

/// Build a tracing directive setting the log level for the relayer crates to the
/// given `log_level`.
pub fn default_directive(log_level: &str) -> String {
    use itertools::Itertools;

    TARGET_CRATES
        .iter()
        .map(|&c| format!("{c}={log_level}"))
        .join(",")
}

/// Builds a tracing filter based on the input `log_levels`.
/// Enables tracing exclusively for the relayer crates.
/// Returns error if the filter failed to build.
fn build_tracing_filter(log_levels: &str) -> EnvFilter {
    // Prefer RUST_LOG as the default setting.
    let mut directive = EnvFilter::from_default_env();

    if !log_levels.is_empty() {
        for log_level in log_levels.split(',') {
            // app_log_level: no target means only the application log should be targeted
            // https://github.com/informalsystems/malachite/pull/287#discussion_r1684212675
            let app_log_level = if !log_level.contains('=') {
                default_directive(log_level)
            } else {
                log_level.to_string()
            }
            .parse()
            .unwrap_or_else(|e| panic!("Invalid log level '{log_level}': {e}"));

            directive = directive.add_directive(app_log_level)
        }
    }

    directive
}

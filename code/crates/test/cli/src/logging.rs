use std::sync::OnceLock;

use tracing::error;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, reload, Registry};

use malachitebft_config::LogFormat;

pub use malachitebft_config::LogLevel;
pub use tracing_subscriber::filter::EnvFilter;

static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();
static DEFAULT_LOG_LEVEL: OnceLock<String> = OnceLock::new();

pub fn reset() {
    let log_level = DEFAULT_LOG_LEVEL
        .get()
        .expect("failed to get the default log level");

    reload_env_filter(build_tracing_filter(log_level));
}

pub fn reload(log_level: LogLevel) {
    let env_filter = build_tracing_filter(&log_level.to_string());
    reload_env_filter(env_filter);
}

fn reload_env_filter(env_filter: EnvFilter) {
    if let Some(handle) = RELOAD_HANDLE.get() {
        if let Err(e) = handle.reload(env_filter) {
            error!("Failed to reload the log level: {e}");
        }
    } else {
        error!("ERROR: Failed to get the reload handle");
    }
}

/// Initialize logging.
///
/// Returns a drop guard responsible for flushing any remaining logs when the program terminates.
/// The guard must be assigned to a binding that is not _, as _ will result in the guard being dropped immediately.
pub fn init(log_level: LogLevel, log_format: LogFormat) -> WorkerGuard {
    let log_level = if let Ok(rust_log) = std::env::var("RUST_LOG") {
        rust_log
    } else {
        log_level.to_string()
    };

    DEFAULT_LOG_LEVEL
        .set(log_level.clone())
        .expect("failed to set the default log level");

    let env_filter = build_tracing_filter(&log_level);

    let (reload_filter, reload_handle) = reload::Layer::new(env_filter);
    RELOAD_HANDLE
        .set(reload_handle)
        .expect("failed to set the reload handle");

    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let fmt_layer = fmt::Layer::default()
        .with_target(false)
        .with_writer(non_blocking)
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    // There must be a better way to use conditionals in the builder pattern.
    match log_format {
        LogFormat::Plaintext => {
            tracing_subscriber::registry()
                .with(reload_filter)
                .with(fmt_layer)
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(reload_filter)
                .with(fmt_layer.json())
                .init();
        }
    };

    guard
}

/// Checks if output is going to a terminal.
///
/// Determines if both stdout and stderr are proper terminals (TTY).
/// This helps decide whether to enable colored output with ANSI escape codes.
/// Colors are disabled when output is redirected to a file.
pub fn enable_ansi() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
}

/// Common prefixes of the crates targeted by the default log level.
const TARGET_CRATES: &[&str] = &["informalsystems_malachitebft"];

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

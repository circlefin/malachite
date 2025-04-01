pub fn init_logging() {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let debug_vars = &[("ACTIONS_RUNNER_DEBUG", "true"), ("MALACHITE_DEBUG", "1")];
    let enable_debug = debug_vars
        .iter()
        .any(|(k, v)| std::env::var(k).as_deref() == Ok(v));

    let trace_level = if enable_debug { "debug" } else { "info" };

    let directives = &[
        ("informalsystems_malachitebft", trace_level),
        (env!("CARGO_CRATE_NAME"), "debug"),
        ("it", "debug"), // Name of the integration test crate
        ("informalsystems_malachitebft_test", "debug"),
        ("informalsystems_malachitebft_test_app", "debug"),
        ("informalsystems_malachitebft_discovery", "error"),
        ("libp2p", "warn"),
        ("ractor", "warn"),
    ];

    let directive = directives
        .iter()
        .map(|(target, level)| format!("{target}={level}"))
        .collect::<Vec<_>>()
        .join(",");

    let filter = EnvFilter::builder().parse(directive).unwrap();

    pub fn enable_ansi() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
    }

    // Construct a tracing subscriber with the supplied filter and enable reloading.
    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_test_writer()
        .with_ansi(enable_ansi())
        .with_thread_ids(false);

    let subscriber = builder.finish();

    if let Err(e) = subscriber.try_init() {
        eprintln!("Failed to initialize logging: {e}");
    }
}

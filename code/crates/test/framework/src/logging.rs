pub fn init_logging() {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let crate_name = env!("CARGO_CRATE_NAME");
    let debug_vars = &[("ACTIONS_RUNNER_DEBUG", "true"), ("MALACHITE_DEBUG", "1")];
    let enable_debug = debug_vars
        .iter()
        .any(|(k, v)| std::env::var(k).as_deref() == Ok(v));

    let trace_level = if enable_debug { "trace" } else { "info" };
    let directive = format!(
        "{crate_name}=debug,informalsystems_malachitebft={trace_level},informalsystems_malachitebft_discovery=error,libp2p=warn,ractor=warn"
    );

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

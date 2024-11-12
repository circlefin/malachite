use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

use malachite_config::MetricsConfig;

#[tracing::instrument(name = "metrics", skip_all)]
pub async fn serve(config: MetricsConfig) {
    let app = Router::new().route("/metrics", get(get_metrics));
    let listener = TcpListener::bind(config.listen_addr).await.unwrap();

    info!(address = %config.listen_addr, "Serving metrics");
    axum::serve(listener, app).await.unwrap();
}

async fn get_metrics() -> String {
    let mut buf = String::new();
    malachite_metrics::export(&mut buf);
    buf
}

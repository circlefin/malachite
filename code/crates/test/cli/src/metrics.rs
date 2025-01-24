use std::io;

use axum::routing::get;
use axum::Router;
use tokio::net::{TcpListener, ToSocketAddrs};
use tracing::{error, info};

use malachitebft_app::metrics::export;

#[tracing::instrument(name = "metrics", skip_all)]
pub async fn serve(listen_addr: impl ToSocketAddrs) {
    if let Err(e) = inner(listen_addr).await {
        error!("Metrics server failed: {e}");
    }
}

async fn inner(listen_addr: impl ToSocketAddrs) -> io::Result<()> {
    let app = Router::new().route("/metrics", get(get_metrics));
    let listener = TcpListener::bind(listen_addr).await?;
    let local_addr = listener.local_addr()?;

    info!(address = %local_addr, "Serving metrics");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_metrics() -> String {
    let mut buf = String::new();
    export(&mut buf);
    buf
}

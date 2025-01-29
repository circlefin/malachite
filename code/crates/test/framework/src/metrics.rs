use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

#[tracing::instrument(name = "metrics", skip_all)]
pub async fn serve_metrics(listen_addr: SocketAddr) {
    let app = Router::new().route("/metrics", get(get_metrics));
    let listener = TcpListener::bind(listen_addr).await.unwrap();
    let address = listener.local_addr().unwrap();

    async fn get_metrics() -> String {
        let mut buf = String::new();
        malachitebft_metrics::export(&mut buf);
        buf
    }

    info!(%address, "Serving metrics");
    axum::serve(listener, app).await.unwrap();
}

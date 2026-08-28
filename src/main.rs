use axum::{
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tracing::info;

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "titan-control-plane"
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".into())
        )
        .init();

    let app = Router::new()
        .route("/health", get(health));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    info!("Titan control plane listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind port 8080");

    axum::serve(listener, app)
        .await
        .expect("Titan server stopped");
}

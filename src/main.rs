use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use titan_core::{
    AgentHeartbeat, AgentRole, MonitorConfig, MonitorService, OperatorReleaseRequest,
    TransactionIntakeRequest,
};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone)]
struct AppState {
    monitor: Arc<RwLock<MonitorService>>,
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let monitor = state.monitor.read().await;
    Json(json!({
        "status": "ok",
        "service": "titan-control-plane",
        "ready": monitor.is_ready(),
        "active_agents": monitor.active_agents(),
    }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let monitor = state.monitor.read().await;
    if monitor.is_ready() {
        Ok(Json(json!({
            "status": "ready"
        })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

async fn status(State(state): State<AppState>) -> Json<titan_core::MonitorStatus> {
    let monitor = state.monitor.read().await;
    Json(monitor.status())
}

async fn alerts(State(state): State<AppState>) -> Json<Vec<titan_core::TransactionAlert>> {
    let monitor = state.monitor.read().await;
    Json(monitor.alerts().to_vec())
}

async fn held_transactions(
    State(state): State<AppState>,
) -> Json<Vec<titan_core::StablecoinTransaction>> {
    let monitor = state.monitor.read().await;
    Json(monitor.held_transactions())
}

async fn ingest_transaction(
    State(state): State<AppState>,
    Json(payload): Json<TransactionIntakeRequest>,
) -> (StatusCode, Json<titan_core::TransactionAlert>) {
    let mut monitor = state.monitor.write().await;
    let alert = monitor.ingest(payload);
    (StatusCode::ACCEPTED, Json(alert))
}

async fn release_transaction(
    State(state): State<AppState>,
    Path(transaction_id): Path<String>,
    Json(payload): Json<OperatorReleaseRequest>,
) -> Result<Json<titan_core::StablecoinTransaction>, StatusCode> {
    let mut monitor = state.monitor.write().await;
    monitor
        .release(&transaction_id, payload)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn register_heartbeat(
    State(state): State<AppState>,
    Json(payload): Json<AgentHeartbeat>,
) -> StatusCode {
    let mut monitor = state.monitor.write().await;
    monitor.register_or_update_agent(payload);
    StatusCode::ACCEPTED
}

async fn report_agent_failure(
    State(state): State<AppState>,
    Path(agent_name): Path<String>,
) -> StatusCode {
    let mut monitor = state.monitor.write().await;
    if monitor.mark_agent_failure(&agent_name) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::NOT_FOUND
    }
}

fn monitor_from_env() -> MonitorService {
    let safe_wallet = std::env::var("SAFE_WALLET_ADDRESS")
        .unwrap_or_else(|_| "SAFE-WALLET-UNCONFIGURED".into());
    let approved_destinations = std::env::var("APPROVED_DESTINATIONS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let minimum_active_agents = std::env::var("MINIMUM_ACTIVE_AGENTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2);

    let mut monitor = MonitorService::new(MonitorConfig {
        safe_wallet,
        approved_destinations,
        minimum_active_agents,
    });

    for (name, role) in [
        ("sentinel-intake", AgentRole::Intake),
        ("sentinel-risk-review", AgentRole::RiskReview),
        ("sentinel-release-coordinator", AgentRole::ReleaseCoordinator),
    ] {
        monitor.register_or_update_agent(AgentHeartbeat {
            agent_name: name.into(),
            role,
        });
    }

    monitor
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let app_state = AppState {
        monitor: Arc::new(RwLock::new(monitor_from_env())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/status", get(status))
        .route("/alerts", get(alerts))
        .route("/transactions/held", get(held_transactions))
        .route("/transactions/intake", post(ingest_transaction))
        .route("/transactions/:transaction_id/release", post(release_transaction))
        .route("/agents/heartbeat", post(register_heartbeat))
        .route("/agents/:agent_name/failure", post(report_agent_failure))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    info!("Titan control plane listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind port 8080");

    axum::serve(listener, app)
        .await
        .expect("Titan server stopped");
}

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use titan_core::{
    AgentHeartbeat, AgentRole, MonitorConfig, MonitorService, MonitorSnapshot, MonitorStatus,
    OperatorReleaseRequest, TransactionAlert, TransactionIntakeRequest,
};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone)]
struct AppState {
    monitor: Arc<RwLock<MonitorService>>,
    operator_api_token: Option<String>,
    monitor_state_path: Option<PathBuf>,
}

#[derive(Clone)]
struct RuntimeConfig {
    operator_api_token: Option<String>,
    monitor_state_path: Option<PathBuf>,
}

fn operator_api_token_from_env() -> Option<String> {
    std::env::var("OPERATOR_API_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn monitor_state_path_from_env() -> Option<PathBuf> {
    std::env::var("MONITOR_STATE_PATH")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn runtime_config_from_env() -> RuntimeConfig {
    RuntimeConfig {
        operator_api_token: operator_api_token_from_env(),
        monitor_state_path: monitor_state_path_from_env(),
    }
}

fn expected_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ")
}

fn is_authorized(headers: &HeaderMap, expected_token: Option<&str>) -> bool {
    match expected_token {
        Some(expected_token) => expected_bearer_token(headers) == Some(expected_token),
        None => false,
    }
}

async fn persist_snapshot(path: &PathBuf, snapshot: &MonitorSnapshot) -> Result<(), StatusCode> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let body = serde_json::to_vec_pretty(snapshot).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tokio::fs::write(path, body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
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

async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let monitor = state.monitor.read().await;
    let monitor_status: MonitorStatus = monitor.status();
    Json(json!({
        "monitor": monitor_status,
        "operator_auth_configured": state.operator_api_token.is_some(),
        "persistence_enabled": state.monitor_state_path.is_some(),
    }))
}

async fn alerts(State(state): State<AppState>) -> Json<Vec<TransactionAlert>> {
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
    let snapshot = state
        .monitor_state_path
        .as_ref()
        .map(|_| monitor.snapshot());
    drop(monitor);
    if let (Some(path), Some(snapshot)) = (&state.monitor_state_path, snapshot.as_ref()) {
        if persist_snapshot(path, snapshot).await.is_err() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TransactionAlert {
                    transaction_id: alert.transaction_id,
                    findings: alert.findings,
                    requires_operator_review: true,
                }),
            );
        }
    }
    (StatusCode::ACCEPTED, Json(alert))
}

async fn release_transaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(transaction_id): Path<String>,
    Json(payload): Json<OperatorReleaseRequest>,
) -> Result<Json<titan_core::StablecoinTransaction>, StatusCode> {
    if !is_authorized(&headers, state.operator_api_token.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut monitor = state.monitor.write().await;
    let released = monitor
        .release(&transaction_id, payload)
        .ok_or(StatusCode::NOT_FOUND)?;
    let snapshot = state
        .monitor_state_path
        .as_ref()
        .map(|_| monitor.snapshot());
    drop(monitor);
    if let (Some(path), Some(snapshot)) = (&state.monitor_state_path, snapshot.as_ref()) {
        persist_snapshot(path, snapshot).await?;
    }
    Ok(Json(released))
}

async fn register_heartbeat(
    State(state): State<AppState>,
    Json(payload): Json<AgentHeartbeat>,
) -> StatusCode {
    let mut monitor = state.monitor.write().await;
    monitor.register_or_update_agent(payload);
    let snapshot = state
        .monitor_state_path
        .as_ref()
        .map(|_| monitor.snapshot());
    drop(monitor);
    if let (Some(path), Some(snapshot)) = (&state.monitor_state_path, snapshot.as_ref()) {
        if persist_snapshot(path, snapshot).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    StatusCode::ACCEPTED
}

async fn report_agent_failure(
    State(state): State<AppState>,
    Path(agent_name): Path<String>,
) -> StatusCode {
    let mut monitor = state.monitor.write().await;
    if monitor.mark_agent_failure(&agent_name) {
        let snapshot = state
            .monitor_state_path
            .as_ref()
            .map(|_| monitor.snapshot());
        drop(monitor);
        if let (Some(path), Some(snapshot)) = (&state.monitor_state_path, snapshot.as_ref()) {
            if persist_snapshot(path, snapshot).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
        StatusCode::ACCEPTED
    } else {
        StatusCode::NOT_FOUND
    }
}

fn port_from_env() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080)
}

fn monitor_config_from_env() -> MonitorConfig {
    let safe_wallet =
        std::env::var("SAFE_WALLET_ADDRESS").unwrap_or_else(|_| "SAFE-WALLET-UNCONFIGURED".into());
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

    MonitorConfig {
        safe_wallet,
        approved_destinations,
        minimum_active_agents,
    }
}

fn seed_agents(monitor: &mut MonitorService) {
    for (name, role) in [
        ("sentinel-intake", AgentRole::Intake),
        ("sentinel-risk-review", AgentRole::RiskReview),
        (
            "sentinel-release-coordinator",
            AgentRole::ReleaseCoordinator,
        ),
    ] {
        monitor.register_or_update_agent(AgentHeartbeat {
            agent_name: name.into(),
            role,
        });
    }
}

async fn monitor_from_env(runtime_config: &RuntimeConfig) -> MonitorService {
    let config = monitor_config_from_env();
    let loaded_snapshot = if let Some(path) = &runtime_config.monitor_state_path {
        tokio::fs::read(path)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice::<MonitorSnapshot>(&bytes).ok())
    } else {
        None
    };

    if let Some(snapshot) = loaded_snapshot {
        let mut monitor = MonitorService::from_snapshot(snapshot);
        monitor.replace_config(config);
        seed_agents(&mut monitor);
        monitor
    } else {
        let mut monitor = MonitorService::new(config);
        seed_agents(&mut monitor);
        monitor
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let runtime_config = runtime_config_from_env();
    let app_state = AppState {
        monitor: Arc::new(RwLock::new(monitor_from_env(&runtime_config).await)),
        operator_api_token: runtime_config.operator_api_token,
        monitor_state_path: runtime_config.monitor_state_path,
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/status", get(status))
        .route("/alerts", get(alerts))
        .route("/transactions/held", get(held_transactions))
        .route("/transactions/intake", post(ingest_transaction))
        .route(
            "/transactions/:transaction_id/release",
            post(release_transaction),
        )
        .route("/agents/heartbeat", post(register_heartbeat))
        .route("/agents/:agent_name/failure", post(report_agent_failure))
        .with_state(app_state);

    let port = port_from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Titan control plane listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|error| panic!("failed to bind port {port}: {error}"));

    axum::serve(listener, app)
        .await
        .expect("Titan server stopped");
}

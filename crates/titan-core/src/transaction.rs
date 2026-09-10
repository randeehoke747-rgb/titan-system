use crate::{AgentRole, Vulnerability};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Stablecoin {
    Usdc,
    Usdt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionState {
    Held,
    Released,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StablecoinTransaction {
    pub transaction_id: String,
    pub asset: Stablecoin,
    pub amount_cents: u64,
    pub source_wallet: String,
    pub destination_wallet: String,
    pub safe_wallet: String,
    pub state: TransactionState,
    pub hold_reason: String,
    pub created_at_epoch_ms: u128,
    pub released_by: Option<String>,
    pub release_note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionIntakeRequest {
    pub transaction_id: Option<String>,
    pub asset: Stablecoin,
    pub amount_cents: u64,
    pub source_wallet: String,
    pub destination_wallet: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorReleaseRequest {
    pub operator_id: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentHeartbeat {
    pub agent_name: String,
    pub role: AgentRole,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorConfig {
    pub safe_wallet: String,
    pub approved_destinations: Vec<String>,
    pub minimum_active_agents: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionAlert {
    pub transaction_id: String,
    pub findings: Vec<Vulnerability>,
    pub requires_operator_review: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorStatus {
    pub safe_wallet: String,
    pub safe_wallet_configured: bool,
    pub active_agents: usize,
    pub minimum_active_agents: usize,
    pub ready: bool,
    pub queued_transactions: usize,
    pub held_transactions: usize,
    pub released_transactions: usize,
    pub last_release_operator: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorSnapshot {
    pub config: MonitorConfig,
    pub agents: Vec<crate::AutonomousAgent>,
    pub transactions: Vec<StablecoinTransaction>,
    pub alerts: Vec<TransactionAlert>,
    pub audit_events: Vec<AuditEvent>,
    pub last_release_operator: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    TransactionHeld,
    TransactionReleased,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub event_id: String,
    pub transaction_id: String,
    pub kind: AuditEventKind,
    pub detail: String,
    pub recorded_at_epoch_ms: u128,
}

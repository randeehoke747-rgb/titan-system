use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    AgentHeartbeat, AutonomousAgent, KeyCompromiseDetector, MonitorConfig, MonitorSnapshot,
    MonitorStatus, OperatorReleaseRequest, StablecoinTransaction, TransactionAlert,
    TransactionIntakeRequest, TransactionState,
};

#[derive(Clone, Debug)]
pub struct MonitorService {
    config: MonitorConfig,
    detector: KeyCompromiseDetector,
    agents: HashMap<String, AutonomousAgent>,
    transactions: HashMap<String, StablecoinTransaction>,
    alerts: Vec<TransactionAlert>,
    last_release_operator: Option<String>,
}

impl MonitorService {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            detector: KeyCompromiseDetector::new(config.approved_destinations.clone()),
            config,
            agents: HashMap::new(),
            transactions: HashMap::new(),
            alerts: Vec::new(),
            last_release_operator: None,
        }
    }

    pub fn from_snapshot(snapshot: MonitorSnapshot) -> Self {
        let mut service = Self::new(snapshot.config);
        service.agents = snapshot
            .agents
            .into_iter()
            .map(|agent| (agent.name.clone(), agent))
            .collect();
        service.transactions = snapshot
            .transactions
            .into_iter()
            .map(|transaction| (transaction.transaction_id.clone(), transaction))
            .collect();
        service.alerts = snapshot.alerts;
        service.last_release_operator = snapshot.last_release_operator;
        service
    }

    pub fn snapshot(&self) -> MonitorSnapshot {
        MonitorSnapshot {
            config: self.config.clone(),
            agents: self.agents.values().cloned().collect(),
            transactions: self.transactions.values().cloned().collect(),
            alerts: self.alerts.clone(),
            last_release_operator: self.last_release_operator.clone(),
        }
    }

    pub fn replace_config(&mut self, config: MonitorConfig) {
        self.detector = KeyCompromiseDetector::new(config.approved_destinations.clone());
        self.config = config;
    }

    pub fn register_or_update_agent(&mut self, heartbeat: AgentHeartbeat) {
        let agent_name = heartbeat.agent_name;
        let role = heartbeat.role;
        let entry = self
            .agents
            .entry(agent_name.clone())
            .or_insert_with(|| AutonomousAgent::new(agent_name, role.clone()));
        entry.role = role;
        entry.recover();
    }

    pub fn mark_agent_failure(&mut self, agent_name: &str) -> bool {
        if let Some(agent) = self.agents.get_mut(agent_name) {
            agent.mark_failure();
            return true;
        }

        false
    }

    pub fn ingest(&mut self, request: TransactionIntakeRequest) -> TransactionAlert {
        let transaction_id = request.transaction_id.unwrap_or_else(|| {
            format!(
                "{}-{}",
                request.source_wallet.replace(' ', ""),
                self.transactions.len() + 1
            )
        });

        let transaction = StablecoinTransaction {
            transaction_id: transaction_id.clone(),
            asset: request.asset,
            amount_cents: request.amount_cents,
            source_wallet: request.source_wallet,
            destination_wallet: request.destination_wallet,
            safe_wallet: self.config.safe_wallet.clone(),
            state: TransactionState::Held,
            hold_reason: "Automatic temporary hold pending operator-supervised release.".into(),
            created_at_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            released_by: None,
            release_note: None,
        };

        let findings = self.detector.inspect(&transaction);
        let alert = TransactionAlert {
            transaction_id: transaction_id.clone(),
            requires_operator_review: !findings.is_empty(),
            findings,
        };

        if alert.requires_operator_review {
            self.alerts.push(alert.clone());
        }

        self.transactions.insert(transaction_id, transaction);
        alert
    }

    pub fn release(
        &mut self,
        transaction_id: &str,
        request: OperatorReleaseRequest,
    ) -> Option<StablecoinTransaction> {
        let transaction = self.transactions.get_mut(transaction_id)?;
        transaction.state = TransactionState::Released;
        transaction.released_by = Some(request.operator_id.clone());
        transaction.release_note = request.note;
        self.last_release_operator = Some(request.operator_id);
        Some(transaction.clone())
    }

    pub fn alerts(&self) -> &[TransactionAlert] {
        &self.alerts
    }

    pub fn held_transactions(&self) -> Vec<StablecoinTransaction> {
        self.transactions
            .values()
            .filter(|transaction| transaction.state == TransactionState::Held)
            .cloned()
            .collect()
    }

    pub fn status(&self) -> MonitorStatus {
        let held_transactions = self
            .transactions
            .values()
            .filter(|transaction| transaction.state == TransactionState::Held)
            .count();
        let released_transactions = self
            .transactions
            .values()
            .filter(|transaction| transaction.state == TransactionState::Released)
            .count();

        MonitorStatus {
            safe_wallet: self.config.safe_wallet.clone(),
            active_agents: self.active_agents(),
            minimum_active_agents: self.config.minimum_active_agents,
            ready: self.is_ready(),
            queued_transactions: self.transactions.len(),
            held_transactions,
            released_transactions,
            last_release_operator: self.last_release_operator.clone(),
        }
    }

    pub fn is_ready(&self) -> bool {
        !self.config.safe_wallet.trim().is_empty()
            && self.active_agents() >= self.config.minimum_active_agents
    }

    pub fn active_agents(&self) -> usize {
        self.agents
            .values()
            .filter(|agent| agent.is_healthy())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AgentHeartbeat, AgentRole, MonitorConfig, MonitorService, Stablecoin,
        TransactionIntakeRequest,
    };

    #[test]
    fn intake_places_transaction_on_hold_in_safe_wallet() {
        let mut service = MonitorService::new(MonitorConfig {
            safe_wallet: "safe-wallet".into(),
            approved_destinations: vec!["customer-wallet".into()],
            minimum_active_agents: 2,
        });
        service.register_or_update_agent(AgentHeartbeat {
            agent_name: "agent-a".into(),
            role: AgentRole::Intake,
        });
        service.register_or_update_agent(AgentHeartbeat {
            agent_name: "agent-b".into(),
            role: AgentRole::RiskReview,
        });

        let alert = service.ingest(TransactionIntakeRequest {
            transaction_id: Some("tx-1".into()),
            asset: Stablecoin::Usdc,
            amount_cents: 1_500,
            source_wallet: "source-wallet".into(),
            destination_wallet: "customer-wallet".into(),
        });

        assert!(alert.requires_operator_review);
        assert_eq!(service.alerts().len(), 1);
        let held = service.held_transactions();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].safe_wallet, "safe-wallet");
    }

    #[test]
    fn usdc_transactions_are_always_high_alert() {
        let mut service = MonitorService::new(MonitorConfig {
            safe_wallet: "safe-wallet".into(),
            approved_destinations: vec!["approved-wallet".into()],
            minimum_active_agents: 1,
        });

        let alert = service.ingest(TransactionIntakeRequest {
            transaction_id: Some("tx-usdc".into()),
            asset: Stablecoin::Usdc,
            amount_cents: 10_000,
            source_wallet: "source-wallet".into(),
            destination_wallet: "approved-wallet".into(),
        });

        assert!(alert.requires_operator_review);
        assert!(alert
            .findings
            .iter()
            .any(|finding| finding.code == "usdc_high_alert"));
    }

    #[test]
    fn unsupported_destination_creates_alert() {
        let mut service = MonitorService::new(MonitorConfig {
            safe_wallet: "safe-wallet".into(),
            approved_destinations: vec!["approved-wallet".into()],
            minimum_active_agents: 1,
        });

        let alert = service.ingest(TransactionIntakeRequest {
            transaction_id: Some("tx-2".into()),
            asset: Stablecoin::Usdt,
            amount_cents: 25_000_000,
            source_wallet: "source-wallet".into(),
            destination_wallet: "unknown-wallet".into(),
        });

        assert!(alert.requires_operator_review);
        assert_eq!(service.alerts().len(), 1);
    }

    #[test]
    fn readiness_requires_minimum_healthy_agents() {
        let mut service = MonitorService::new(MonitorConfig {
            safe_wallet: "safe-wallet".into(),
            approved_destinations: vec![],
            minimum_active_agents: 2,
        });

        service.register_or_update_agent(AgentHeartbeat {
            agent_name: "agent-a".into(),
            role: AgentRole::Intake,
        });
        service.register_or_update_agent(AgentHeartbeat {
            agent_name: "agent-b".into(),
            role: AgentRole::RiskReview,
        });

        assert!(service.is_ready());
        assert!(service.mark_agent_failure("agent-a"));
        assert!(service.mark_agent_failure("agent-a"));
        assert!(service.mark_agent_failure("agent-a"));
        assert!(!service.is_ready());
    }

    #[test]
    fn snapshot_round_trip_preserves_transactions() {
        let mut service = MonitorService::new(MonitorConfig {
            safe_wallet: "safe-wallet".into(),
            approved_destinations: vec!["approved-wallet".into()],
            minimum_active_agents: 1,
        });
        service.register_or_update_agent(AgentHeartbeat {
            agent_name: "agent-a".into(),
            role: AgentRole::Intake,
        });
        service.ingest(TransactionIntakeRequest {
            transaction_id: Some("tx-3".into()),
            asset: Stablecoin::Usdc,
            amount_cents: 5_000,
            source_wallet: "source-wallet".into(),
            destination_wallet: "approved-wallet".into(),
        });

        let restored = MonitorService::from_snapshot(service.snapshot());

        assert_eq!(restored.held_transactions().len(), 1);
        assert_eq!(restored.active_agents(), 1);
    }
}

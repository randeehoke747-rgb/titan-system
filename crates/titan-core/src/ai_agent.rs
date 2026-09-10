use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Intake,
    RiskReview,
    ReleaseCoordinator,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutonomousAgent {
    pub name: String,
    pub role: AgentRole,
    pub active: bool,
    pub consecutive_failures: u32,
}

impl AutonomousAgent {
    pub fn new(name: impl Into<String>, role: AgentRole) -> Self {
        Self {
            name: name.into(),
            role,
            active: true,
            consecutive_failures: 0,
        }
    }

    pub fn mark_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= 3 {
            self.active = false;
        }
    }

    pub fn recover(&mut self) {
        self.active = true;
        self.consecutive_failures = 0;
    }

    pub fn is_healthy(&self) -> bool {
        self.active && self.consecutive_failures < 3
    }
}

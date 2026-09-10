use std::collections::HashSet;

use crate::{StablecoinTransaction, Vulnerability, VulnerabilityLevel};

#[derive(Clone, Debug)]
pub struct KeyCompromiseDetector {
    approved_destinations: HashSet<String>,
    high_value_threshold_cents: u64,
}

impl KeyCompromiseDetector {
    pub fn new(approved_destinations: impl IntoIterator<Item = String>) -> Self {
        Self {
            approved_destinations: approved_destinations.into_iter().collect(),
            high_value_threshold_cents: 10_000_000,
        }
    }

    pub fn inspect(&self, transaction: &StablecoinTransaction) -> Vec<Vulnerability> {
        let mut findings = Vec::new();

        if transaction.amount_cents == 0 {
            findings.push(Vulnerability::new(
                "zero_amount",
                "Zero-value transactions should be reviewed before release.",
                VulnerabilityLevel::Medium,
            ));
        }

        if transaction.source_wallet == transaction.destination_wallet {
            findings.push(Vulnerability::new(
                "same_source_destination",
                "Source and destination wallets should not be identical.",
                VulnerabilityLevel::High,
            ));
        }

        if !self.approved_destinations.is_empty()
            && !self
                .approved_destinations
                .contains(&transaction.destination_wallet)
        {
            findings.push(Vulnerability::new(
                "destination_not_allowlisted",
                "Destination wallet is not on the approved allowlist.",
                VulnerabilityLevel::Critical,
            ));
        }

        if transaction.amount_cents >= self.high_value_threshold_cents {
            findings.push(Vulnerability::new(
                "high_value_transfer",
                "High-value stablecoin transfer exceeds the review threshold.",
                VulnerabilityLevel::High,
            ));
        }

        findings
    }
}

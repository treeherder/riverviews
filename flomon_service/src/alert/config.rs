/// Alert notification configuration loader.
///
/// Parses `alerting.toml` from the current working directory.

use serde::Deserialize;
use std::fs;

/// Root structure matching alerting.toml
#[derive(Debug, Clone, Deserialize)]
pub struct AlertingConfig {
    pub alerting: AlertingSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertingSection {
    pub enabled: bool,
    pub pubsub_project: String,
    pub pubsub_topic: String,
    pub pubsub_enabled: bool,
    pub daily_digest_hour_utc: i32,
    pub intervals_minutes: IntervalsConfig,
    pub recipients: RecipientsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IntervalsConfig {
    pub action: u64,
    pub flood: u64,
    pub moderate: u64,
    pub major: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecipientsConfig {
    pub numbers: Vec<String>,
}

impl AlertingConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string("alerting.toml")
            .map_err(|e| format!("Failed to read alerting.toml: {}", e))?;
        let config: AlertingConfig = toml::from_str(&contents)
            .map_err(|e| format!("Failed to parse alerting.toml: {}", e))?;
        Ok(config)
    }
}

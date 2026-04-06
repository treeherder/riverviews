/// Alert notification orchestrator.
///
/// `Notifier` wraps AlertStateStore, AlertingConfig, and the Pub/Sub client
/// into a single object the daemon calls after each poll cycle.  It decides:
///
/// - Whether a given site warrants a notification (via AlertStateStore).
/// - Formats the SMS body from the FloodAlert.
/// - Publishes to Pub/Sub (or logs when pubsub_enabled = false).
///
/// The daemon should call `process_reading_alert` for every USGS stage reading
/// and `send_daily_digest` once per day if a digest is configured.

use crate::alert::config::AlertingConfig;
use crate::alert::pubsub::{self, AlertMessage};
use crate::alert::state::AlertStateStore;
use crate::alert::thresholds::{check_flood_stage, FloodAlert, FloodSeverity};
use crate::model::{FloodThresholds, GaugeReading};
use chrono::Utc;
use std::error::Error;

pub struct Notifier {
    config: AlertingConfig,
    state: AlertStateStore,
    http: reqwest::blocking::Client,
}

impl Notifier {
    /// Load configuration from alerting.toml and initialise state.
    ///
    /// Returns `None` (with a warning) if alerting.toml is missing or
    /// malformed so the daemon can continue without alerting.
    pub fn try_load() -> Option<Self> {
        match AlertingConfig::load() {
            Ok(config) => {
                if !config.alerting.enabled {
                    println!("ℹ Alerting disabled in alerting.toml — skipping notifications");
                    return None;
                }
                let http = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                    .expect("Failed to build HTTP client for alerting");
                println!(
                    "🔔 Alerting enabled — Pub/Sub project={} topic={} dry_run={}",
                    config.alerting.pubsub_project,
                    config.alerting.pubsub_topic,
                    !config.alerting.pubsub_enabled,
                );
                Some(Self {
                    config,
                    state: AlertStateStore::new(),
                    http,
                })
            }
            Err(e) => {
                eprintln!("Warning: Could not load alerting.toml ({}) — alerts disabled", e);
                None
            }
        }
    }

    /// Evaluate a stage reading against thresholds and send a notification
    /// if one is warranted by the cooldown/deduplication logic.
    ///
    /// No-op (soft fail with log) if Pub/Sub publish fails so the daemon
    /// polling loop is not interrupted.
    pub fn process_reading_alert(
        &mut self,
        reading: &GaugeReading,
        thresholds: &FloodThresholds,
    ) {
        let alert: Option<FloodAlert> = check_flood_stage(reading, thresholds);
        let current_severity = alert.as_ref().map(|a| &a.severity);

        let interval = self.interval_for(current_severity);
        let now = Utc::now();

        if !self.state.should_notify(&reading.site_code, current_severity, interval, now) {
            return;
        }

        let (body, severity_tag) = match &alert {
            Some(a) => (a.message.clone(), severity_tag(&a.severity)),
            None => (
                format!(
                    "All clear at {} — stage {:.2} ft is below action stage.",
                    reading.site_name, reading.value
                ),
                "all_clear".to_string(),
            ),
        };

        let message = AlertMessage {
            body,
            recipients: self.config.alerting.recipients.numbers.clone(),
            event_time: reading.datetime.clone(),
            severity: severity_tag.clone(),
            site_code: reading.site_code.clone(),
        };

        match pubsub::publish(
            &self.http,
            &self.config.alerting.pubsub_project,
            &self.config.alerting.pubsub_topic,
            &message,
            self.config.alerting.pubsub_enabled,
        ) {
            Ok(_) => {
                self.state.record_notification(
                    &reading.site_code,
                    alert.map(|a| a.severity),
                    now,
                );
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to publish alert for {}: {}",
                    reading.site_code, e
                );
                // Do NOT record the notification — will retry next poll cycle.
            }
        }
    }

    /// Send a daily status digest summarising current conditions across all
    /// provided readings. Call this when the wall-clock UTC hour matches
    /// `daily_digest_hour_utc`.
    pub fn send_daily_digest(
        &self,
        summaries: &[String],
    ) -> Result<(), Box<dyn Error>> {
        if self.config.alerting.daily_digest_hour_utc < 0 {
            return Ok(());
        }

        let body = if summaries.is_empty() {
            "Riverviews daily digest: no active flood alerts. All stations nominal.".to_string()
        } else {
            format!(
                "Riverviews daily digest:\n{}",
                summaries.join("\n")
            )
        };

        let message = AlertMessage {
            body,
            recipients: self.config.alerting.recipients.numbers.clone(),
            event_time: Utc::now().to_rfc3339(),
            severity: "digest".to_string(),
            site_code: "system".to_string(),
        };

        pubsub::publish(
            &self.http,
            &self.config.alerting.pubsub_project,
            &self.config.alerting.pubsub_topic,
            &message,
            self.config.alerting.pubsub_enabled,
        )
    }

    /// Expose configuration for callers (e.g. daemon daily digest scheduling).
    pub fn config(&self) -> &crate::alert::config::AlertingSection {
        &self.config.alerting
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn interval_for(&self, severity: Option<&FloodSeverity>) -> u64 {
        let iv = &self.config.alerting.intervals_minutes;
        match severity {
            None => 0,
            Some(FloodSeverity::Action) => iv.action,
            Some(FloodSeverity::Flood) => iv.flood,
            Some(FloodSeverity::Moderate) => iv.moderate,
            Some(FloodSeverity::Major) => iv.major,
        }
    }
}

fn severity_tag(s: &FloodSeverity) -> String {
    match s {
        FloodSeverity::Action => "action",
        FloodSeverity::Flood => "flood",
        FloodSeverity::Moderate => "moderate",
        FloodSeverity::Major => "major",
    }
    .to_string()
}

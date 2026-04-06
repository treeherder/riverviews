/// Per-site alert state: tracks the last-notified severity and last send time
/// so the daemon can enforce cooldown intervals and suppress duplicate alerts.
///
/// State lives in memory only — on restart the daemon may resend one alert per
/// site at the current severity, which is acceptable (fail-safe: better a
/// duplicate than a missed alert).

use crate::alert::thresholds::FloodSeverity;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Current in-memory alert state for a single monitored site.
#[derive(Debug, Clone)]
pub struct SiteAlertState {
    /// The severity level most recently notified for this site.
    pub last_severity: Option<FloodSeverity>,
    /// When the last notification was sent (used to enforce cooldowns).
    pub last_notified_at: Option<DateTime<Utc>>,
}

impl SiteAlertState {
    pub fn new() -> Self {
        Self {
            last_severity: None,
            last_notified_at: None,
        }
    }
}

/// In-memory store of alert state keyed by site code.
pub struct AlertStateStore {
    sites: HashMap<String, SiteAlertState>,
}

impl AlertStateStore {
    pub fn new() -> Self {
        Self {
            sites: HashMap::new(),
        }
    }

    fn entry(&mut self, site_code: &str) -> &mut SiteAlertState {
        self.sites
            .entry(site_code.to_string())
            .or_insert_with(SiteAlertState::new)
    }

    /// Returns `true` when a notification should be sent for this site.
    ///
    /// Notification is warranted when:
    /// - The severity has gone up (immediate escalation).
    /// - The severity is unchanged but the cooldown interval has elapsed.
    /// - The severity has cleared (all-clear, sent only once per clearance).
    ///
    /// `interval_minutes` is the configured refresh cadence for the current
    /// severity level (0 = severity-change only, no periodic updates).
    pub fn should_notify(
        &mut self,
        site_code: &str,
        current_severity: Option<&FloodSeverity>,
        interval_minutes: u64,
        now: DateTime<Utc>,
    ) -> bool {
        let state = self.entry(site_code);

        match (current_severity, &state.last_severity) {
            // No active alert and none previously — nothing to do.
            (None, None) => false,

            // Alert just cleared — send all-clear once.
            (None, Some(_)) => true,

            // New alert or escalation — always notify.
            (Some(current), None) => {
                let _ = current; // suppress unused warning
                true
            }
            (Some(current), Some(prev)) => {
                if severity_rank(current) > severity_rank(prev) {
                    // Escalation — immediate.
                    return true;
                }
                if severity_rank(current) < severity_rank(prev) {
                    // De-escalation — immediate.
                    return true;
                }
                // Same severity — check cooldown.
                if interval_minutes == 0 {
                    return false;
                }
                match state.last_notified_at {
                    None => true,
                    Some(last) => {
                        let elapsed = (now - last).num_minutes();
                        elapsed >= interval_minutes as i64
                    }
                }
            }
        }
    }

    /// Record that a notification was sent for this site.
    pub fn record_notification(
        &mut self,
        site_code: &str,
        severity: Option<FloodSeverity>,
        now: DateTime<Utc>,
    ) {
        let state = self.entry(site_code);
        state.last_severity = severity;
        state.last_notified_at = Some(now);
    }
}

fn severity_rank(s: &FloodSeverity) -> u8 {
    match s {
        FloodSeverity::Action => 1,
        FloodSeverity::Flood => 2,
        FloodSeverity::Moderate => 3,
        FloodSeverity::Major => 4,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, 5, h, m, 0).unwrap()
    }

    #[test]
    fn no_alert_no_notify() {
        let mut store = AlertStateStore::new();
        assert!(!store.should_notify("site1", None, 60, ts(12, 0)));
    }

    #[test]
    fn new_alert_triggers_notify() {
        let mut store = AlertStateStore::new();
        assert!(store.should_notify(
            "site1",
            Some(&FloodSeverity::Action),
            360,
            ts(12, 0)
        ));
    }

    #[test]
    fn same_severity_within_cooldown_suppressed() {
        let mut store = AlertStateStore::new();
        store.record_notification("site1", Some(FloodSeverity::Action), ts(12, 0));
        // 30 minutes later, 360-minute interval → should not resend
        assert!(!store.should_notify(
            "site1",
            Some(&FloodSeverity::Action),
            360,
            ts(12, 30)
        ));
    }

    #[test]
    fn same_severity_after_cooldown_triggers_notify() {
        let mut store = AlertStateStore::new();
        store.record_notification("site1", Some(FloodSeverity::Action), ts(6, 0));
        // 361 minutes later
        assert!(store.should_notify(
            "site1",
            Some(&FloodSeverity::Action),
            360,
            ts(12, 1)
        ));
    }

    #[test]
    fn escalation_is_immediate() {
        let mut store = AlertStateStore::new();
        store.record_notification("site1", Some(FloodSeverity::Action), ts(12, 0));
        // 1 minute later but severity escalated to Flood
        assert!(store.should_notify(
            "site1",
            Some(&FloodSeverity::Flood),
            180,
            ts(12, 1)
        ));
    }

    #[test]
    fn all_clear_triggers_once() {
        let mut store = AlertStateStore::new();
        store.record_notification("site1", Some(FloodSeverity::Action), ts(12, 0));
        // Alert cleared
        assert!(store.should_notify("site1", None, 360, ts(18, 0)));
        // After recording all-clear, no repeat
        store.record_notification("site1", None, ts(18, 0));
        assert!(!store.should_notify("site1", None, 360, ts(20, 0)));
    }
}

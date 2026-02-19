//! Flood stage threshold checking.
//!
//! Notification dispatch, alert deduplication, and cooldown logic will also likely
//! live here, since they're closely related to the concept of a "threshold breach"
//! and may require access to the same metadata about each site (e.g. which parameters
//! have thresholds, what are the threshold values, etc.).

use crate::model::{FloodThresholds, GaugeReading};

/// Flood severity levels, in ascending order of severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloodSeverity {
    Action,
    Flood,
    Moderate,
    Major,
}

/// A flood alert triggered when a reading exceeds a threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct FloodAlert {
    pub severity: FloodSeverity,
    pub message: String,
}

/// Checks if a stage reading exceeds any flood thresholds and returns an
/// alert if so.
///
/// Returns `None` if the reading is below the action stage threshold.
pub fn check_flood_stage(
    reading: &GaugeReading,
    thresholds: &FloodThresholds,
) -> Option<FloodAlert> {
    // TODO: implement threshold checking logic
    let _ = (reading, thresholds);
    unimplemented!("check_flood_stage: compare reading.value against threshold levels")
}

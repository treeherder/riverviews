//! Flood stage threshold checking.
//!
//! **Simple real-time monitoring** - The daemon compares current readings against
//! static thresholds (NWS flood stages) to generate immediate alerts.
//!
//! **Philosophy:**
//! - Rust daemon: Simple, fast threshold checks for real-time alerts
//! - Python/FloML: Complex analysis to discover better thresholds via segmented regression
//! - Thresholds can be updated based on ML findings for improved accuracy
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
///
/// **Note:** These are simple static thresholds. The ML/analysis layer
/// can discover better thresholds through segmented regression and update
/// these values for more accurate alerts.
pub fn check_flood_stage(
    reading: &GaugeReading,
    thresholds: &FloodThresholds,
) -> Option<FloodAlert> {
    let stage = reading.value;
    
    // Check thresholds in descending order of severity
    if stage >= thresholds.major_flood_stage_ft {
        Some(FloodAlert {
            severity: FloodSeverity::Major,
            message: format!(
                "MAJOR FLOOD at {}: {:.2} ft (major flood stage: {:.2} ft)",
                reading.site_name, stage, thresholds.major_flood_stage_ft
            ),
        })
    } else if stage >= thresholds.moderate_flood_stage_ft {
        Some(FloodAlert {
            severity: FloodSeverity::Moderate,
            message: format!(
                "MODERATE FLOOD at {}: {:.2} ft (moderate flood stage: {:.2} ft)",
                reading.site_name, stage, thresholds.moderate_flood_stage_ft
            ),
        })
    } else if stage >= thresholds.flood_stage_ft {
        Some(FloodAlert {
            severity: FloodSeverity::Flood,
            message: format!(
                "FLOOD at {}: {:.2} ft (flood stage: {:.2} ft)",
                reading.site_name, stage, thresholds.flood_stage_ft
            ),
        })
    } else if stage >= thresholds.action_stage_ft {
        Some(FloodAlert {
            severity: FloodSeverity::Action,
            message: format!(
                "Action stage reached at {}: {:.2} ft (action stage: {:.2} ft)",
                reading.site_name, stage, thresholds.action_stage_ft
            ),
        })
    } else {
        // Below action stage - no alert
        None
    }
}

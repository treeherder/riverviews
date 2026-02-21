/// Site grouping and data organization utilities.
///
/// `group_by_site` takes the flat list of `GaugeReading`s produced by the
/// ingest layer and organizes them into per-site `SiteReadings` structs,
/// making it convenient to ask "what is the current stage at Kingston Mines?"
/// without filtering a flat list every time.
///
/// This module provides basic data organization helpers. Complex analysis
/// such as trend detection, rate-of-rise calculations, and upstream correlation
/// are handled by external Python scripts that operate on the curated database.
///
/// The integration test at the bottom of this module exercises the full
/// parse → group → threshold-check pipeline, and lives here because
/// grouping is the final step in that chain.

use std::collections::HashMap;

use crate::model::{GaugeReading, SiteReadings};

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

/// Groups a flat list of `GaugeReading`s into a map keyed by site code.
///
/// Within each `SiteReadings`, `discharge_cfs` is populated from the reading
/// with `parameter_code == "00060"` and `stage_ft` from `"00065"`. If
/// multiple readings exist for the same site and parameter (which shouldn't
/// happen with a well-formed IV response but could under retry/dedup logic),
/// the last one encountered wins.
pub fn group_by_site(readings: Vec<GaugeReading>) -> HashMap<String, SiteReadings> {
    // TODO: implement — iterate readings, insert or update the SiteReadings
    // entry for each site_code, routing by parameter_code.
    let _ = readings;
    unimplemented!("group_by_site: partition readings into per-site structs")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::thresholds::{check_flood_stage, FloodSeverity};
    use crate::ingest::{fixtures::*, usgs::parse_iv_response};
    use crate::model::FloodThresholds;
    use crate::stations::find_station;

    // --- Grouping: basic correctness ----------------------------------------

    #[test]
    fn test_group_by_site_associates_discharge_and_stage_for_single_site() {
        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);

        let site = grouped
            .get("05568500")
            .expect("Kingston Mines should be in grouped results");

        assert_eq!(site.site_code, "05568500");
        assert!(site.discharge_cfs.is_some(), "should have discharge reading");
        assert!(site.stage_ft.is_some(), "should have stage reading");
    }

    #[test]
    fn test_group_by_site_handles_stage_only_site() {
        // The Peoria pool gauge (05567500) only has stage in the multi-site fixture.
        let readings = parse_iv_response(fixture_multi_site_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);

        let peoria = grouped
            .get("05567500")
            .expect("Peoria pool should be in grouped results");
        assert!(peoria.stage_ft.is_some(), "Peoria pool should have a stage reading");
        assert!(
            peoria.discharge_cfs.is_none(),
            "Peoria pool has no discharge in this fixture"
        );
    }

    #[test]
    fn test_group_by_site_handles_discharge_only_site() {
        // Chillicothe (05568000) only has discharge in the multi-site fixture.
        let readings = parse_iv_response(fixture_multi_site_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);

        let chillicothe = grouped
            .get("05568000")
            .expect("Chillicothe should be in grouped results");
        assert!(chillicothe.discharge_cfs.is_some(), "Chillicothe should have discharge");
        assert!(
            chillicothe.stage_ft.is_none(),
            "Chillicothe has no stage in this fixture"
        );
    }

    #[test]
    fn test_group_by_site_produces_one_entry_per_site() {
        let readings = parse_iv_response(fixture_multi_site_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);
        // Multi-site fixture contains two distinct sites.
        assert_eq!(grouped.len(), 2, "should have exactly 2 site entries");
    }

    #[test]
    fn test_group_by_site_preserves_reading_values() {
        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);

        let site = grouped.get("05568500").expect("Kingston Mines should be present");

        let discharge = site.discharge_cfs.as_ref().expect("should have discharge");
        assert!((discharge.value - 42_300.0).abs() < 0.01);

        let stage = site.stage_ft.as_ref().expect("should have stage");
        assert!((stage.value - 18.42).abs() < 0.001);
    }

    #[test]
    fn test_group_by_site_empty_input_returns_empty_map() {
        let grouped = group_by_site(vec![]);
        assert!(grouped.is_empty(), "empty input should produce empty map");
    }

    // --- Integration: parse → group → threshold check -----------------------

    #[test]
    fn test_pipeline_kingston_mines_18ft_triggers_flood_not_moderate() {
        // Stage of 18.42 ft (from fixture) is above flood (16.0 ft) but
        // below moderate flood (20.0 ft) at Kingston Mines.
        let station = find_station("05568500")
            .expect("Kingston Mines should be in the registry");
        let thresholds = station.thresholds.as_ref()
            .expect("Kingston Mines should have thresholds");

        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("fixture should parse");
        let grouped = group_by_site(readings);
        let site = grouped.get("05568500").expect("Kingston Mines should be present");

        let stage = site.stage_ft.as_ref().expect("should have stage reading");
        let alert = check_flood_stage(stage, thresholds)
            .expect("18.42 ft should trigger a flood alert");

        assert_eq!(
            alert.severity,
            FloodSeverity::Flood,
            "18.42 ft should be Flood severity (not Action, Moderate, or Major)"
        );
        assert!(
            alert.message.to_lowercase().contains("flood"),
            "message should mention 'flood', got: {}",
            alert.message
        );
        assert!(
            !alert.message.to_lowercase().contains("major"),
            "message should not mention 'major' at 18.42 ft"
        );
        assert!(
            !alert.message.to_lowercase().contains("moderate"),
            "message should not mention 'moderate' at 18.42 ft"
        );
    }

    #[test]
    fn test_pipeline_below_action_stage_produces_no_alert() {
        // If we had a fixture with stage < 14.0 ft, no alert should fire.
        // Simulate by checking the Peoria pool gauge at 14.85 ft against
        // Kingston Mines thresholds (action = 14.0) — this should alert.
        // Then verify a sub-threshold value does not.
        let thresholds = FloodThresholds {
            action_stage_ft: 14.0,
            flood_stage_ft: 16.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 24.0,
        };

        // Build a synthetic reading below action stage.
        let low_reading = GaugeReading {
            site_code: "05568500".to_string(),
            site_name: "Illinois River at Kingston Mines, IL".to_string(),
            parameter_code: "00065".to_string(),
            unit: "ft".to_string(),
            value: 12.0,
            datetime: "2024-05-01T12:00:00.000-05:00".to_string(),
            qualifier: "P".to_string(),
        };

        let alert = check_flood_stage(&low_reading, &thresholds);
        assert!(
            alert.is_none(),
            "12.0 ft is well below action stage (14.0 ft), should produce no alert"
        );
    }
}

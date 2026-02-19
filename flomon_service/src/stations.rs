///site code registry, the "valid format" test lives here
/// a map of site codes to metadata (name, location, which parameters are available, NWS flood thresholds). 
/// The site code format test moves here because it's really testing the registry, not the parser. This module will grow as we add more stations and associate threshold data with each one.
/// Station registry for the Peoria basin flood monitoring service.
///
/// Defines the canonical list of USGS gauge stations monitored by this
/// service, along with their metadata and NWS flood stage thresholds.
/// This is the single source of truth for site codes — all other modules
/// should reference stations from here rather than hardcoding site codes.

use crate::model::FloodThresholds;

// ---------------------------------------------------------------------------
// USGS parameter codes (re-exported here for use in URL construction)
// ---------------------------------------------------------------------------

pub use crate::model::{PARAM_DISCHARGE, PARAM_STAGE};

// ---------------------------------------------------------------------------
// Station metadata
// ---------------------------------------------------------------------------

/// Metadata for a single USGS gauge station.
pub struct Station {
    /// 8-digit USGS site code.
    pub site_code: &'static str,
    /// Official USGS site name.
    pub name: &'static str,
    /// Human-readable description of the station's role in flood monitoring.
    pub description: &'static str,
    /// WGS84 latitude.
    pub latitude: f64,
    /// WGS84 longitude.
    pub longitude: f64,
    /// NWS flood stage thresholds, if defined for this station.
    /// Tributary stations may not have official NWS thresholds.
    pub thresholds: Option<FloodThresholds>,
    /// Which parameters this station is expected to provide.
    /// Some stations may only report discharge (00060) or stage (00065).
    pub expected_parameters: &'static [&'static str],
}

/// All USGS gauge stations monitored for Peoria flood risk, ordered
/// roughly from downstream to upstream / main stem to tributary.
///
/// Sources:
///   - Site codes: USGS NWIS (waterservices.usgs.gov)
///   - Flood stages: NWS Advanced Hydrologic Prediction Service (water.noaa.gov)
pub static STATION_REGISTRY: &[Station] = &[
    Station {
        site_code: "05568500",
        name: "Illinois River at Kingston Mines, IL",
        description: "Primary downstream reference gauge just below Peoria. \
                      Use for current flood status at the property.",
        latitude: 40.5614,
        longitude: -89.9956,
        thresholds: Some(FloodThresholds {
            action_stage_ft: 14.0,
            flood_stage_ft: 16.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 24.0,
        }),
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05567500",
        name: "Illinois River at Peoria, IL",
        description: "Pool gauge at Peoria Lock & Dam. Reflects managed pool \
                      level rather than free-flowing stage; use Kingston Mines \
                      for flood stage comparisons.",
        latitude: 40.6939,
        longitude: -89.5898,
        thresholds: None, // pool gauge — NWS thresholds not defined here
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05568000",
        name: "Illinois River at Chillicothe, IL",
        description: "Upstream warning station ~20 miles north of Peoria. \
                      Rising stage here typically leads Peoria by 6–12 hours.",
        latitude: 40.9200,
        longitude: -89.4854,
        thresholds: Some(FloodThresholds {
            action_stage_ft: 13.0,
            flood_stage_ft: 15.0,
            moderate_flood_stage_ft: 19.0,
            major_flood_stage_ft: 23.0,
        }),
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05557000",
        name: "Illinois River at Henry, IL",
        description: "Early warning station ~50 miles upstream. Stage here \
                      leads Peoria by 12–24 hours under typical flow.",
        latitude: 41.1120,
        longitude: -89.3540,
        thresholds: Some(FloodThresholds {
            action_stage_ft: 13.0,
            flood_stage_ft: 15.0,
            moderate_flood_stage_ft: 19.0,
            major_flood_stage_ft: 22.0,
        }),
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05568580",
        name: "Mackinaw River near Green Valley, IL",
        description: "Critical local tributary joining the Illinois near Pekin, \
                      just south of Peoria. Responds quickly to rainfall; \
                      a rising Mackinaw is a strong short-term flood precursor.",
        latitude: 40.7050,
        longitude: -89.6480,
        thresholds: None,
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05570000",
        name: "Spoon River at Seville, IL",
        description: "Upstream tributary joining the Illinois above Havana. \
                      Less directly coupled to Peoria than the Mackinaw but \
                      contributes to overall basin load.",
        latitude: 40.4906,
        longitude: -90.0381,
        thresholds: None,
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05552500",
        name: "Illinois River at Marseilles, IL",
        description: "Main stem gauge near Starved Rock L&D, ~80 miles upstream. \
                      Provides earliest main-stem warning; stage here leads \
                      Peoria by roughly 24–48 hours.",
        latitude: 41.3303,
        longitude: -88.7431,
        thresholds: Some(FloodThresholds {
            action_stage_ft: 12.0,
            flood_stage_ft: 14.0,
            moderate_flood_stage_ft: 18.0,
            major_flood_stage_ft: 22.0,
        }),
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    Station {
        site_code: "05536890",
        name: "Chicago Sanitary & Ship Canal at Romeoville, IL",
        description: "Monitors flow from the Chicago metro area entering the \
                      Illinois River system. Tracks MWRD releases during heavy \
                      rain events, which can spike main-stem flows significantly.",
        latitude: 41.6367,
        longitude: -88.0920,
        thresholds: None,
        expected_parameters: &[PARAM_DISCHARGE], // Canal flow monitoring - stage not meaningful
    },
];

/// Returns the site codes for all monitored stations as a `Vec<&str>`,
/// suitable for passing directly to `ingest::usgs::build_iv_url`.
pub fn all_site_codes() -> Vec<&'static str> {
    STATION_REGISTRY.iter().map(|s| s.site_code).collect()
}

/// Returns site codes that expect a specific parameter.
/// Useful for filtering stations before API requests.
pub fn sites_with_parameter(param_code: &str) -> Vec<&'static str> {
    STATION_REGISTRY
        .iter()
        .filter(|s| s.expected_parameters.contains(&param_code))
        .map(|s| s.site_code)
        .collect()
}

/// Checks if a station is expected to provide a specific parameter.
pub fn station_has_parameter(site_code: &str, param_code: &str) -> bool {
    find_station(site_code)
        .map(|s| s.expected_parameters.contains(&param_code))
        .unwrap_or(false)
}

/// Looks up a station by site code. Returns `None` if not found.
pub fn find_station(site_code: &str) -> Option<&'static Station> {
    STATION_REGISTRY.iter().find(|s| s.site_code == site_code)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_site_codes_are_valid_usgs_format() {
        // USGS site codes for Illinois are 8-digit numeric strings.
        // If any entry in the registry violates this, the IV API will
        // silently drop that site from its response.
        for station in STATION_REGISTRY {
            assert_eq!(
                station.site_code.len(),
                8,
                "site code for '{}' should be 8 digits, got '{}'",
                station.name,
                station.site_code
            );
            assert!(
                station.site_code.chars().all(|c| c.is_ascii_digit()),
                "site code for '{}' should be numeric, got '{}'",
                station.name,
                station.site_code
            );
        }
    }

    #[test]
    fn test_no_duplicate_site_codes() {
        let mut seen = std::collections::HashSet::new();
        for station in STATION_REGISTRY {
            assert!(
                seen.insert(station.site_code),
                "duplicate site code '{}' found in STATION_REGISTRY",
                station.site_code
            );
        }
    }

    #[test]
    fn test_registry_contains_all_expected_peoria_basin_sites() {
        let expected = [
            "05568500", // Kingston Mines (primary)
            "05567500", // Peoria pool
            "05568000", // Chillicothe
            "05557000", // Henry
            "05568580", // Mackinaw River
            "05570000", // Spoon River
            "05552500", // Marseilles / Starved Rock
            "05536890", // Chicago Sanitary & Ship Canal
        ];
        let codes: Vec<_> = STATION_REGISTRY.iter().map(|s| s.site_code).collect();
        for expected_code in &expected {
            assert!(
                codes.contains(expected_code),
                "STATION_REGISTRY missing expected site '{}'",
                expected_code
            );
        }
    }

    #[test]
    fn test_find_station_returns_correct_entry() {
        let station = find_station("05568500").expect("Kingston Mines should be in registry");
        assert_eq!(station.site_code, "05568500");
        assert!(station.name.contains("Kingston Mines"));
    }

    #[test]
    fn test_find_station_returns_none_for_unknown_code() {
        assert!(find_station("00000000").is_none());
    }

    #[test]
    fn test_all_site_codes_helper_matches_registry_length() {
        assert_eq!(all_site_codes().len(), STATION_REGISTRY.len());
    }

    #[test]
    fn test_thresholds_are_ordered_ascending_where_defined() {
        // action < flood < moderate < major — violating this order would
        // cause check_flood_stage to return incorrect severity levels.
        for station in STATION_REGISTRY {
            if let Some(t) = &station.thresholds {
                assert!(
                    t.action_stage_ft < t.flood_stage_ft,
                    "action must be below flood for '{}'",
                    station.name
                );
                assert!(
                    t.flood_stage_ft < t.moderate_flood_stage_ft,
                    "flood must be below moderate for '{}'",
                    station.name
                );
                assert!(
                    t.moderate_flood_stage_ft < t.major_flood_stage_ft,
                    "moderate must be below major for '{}'",
                    station.name
                );
            }
        }
    }

    #[test]
    fn test_parameter_codes_are_valid_and_distinct() {
        assert_eq!(PARAM_DISCHARGE.len(), 5);
        assert_eq!(PARAM_STAGE.len(), 5);
        assert!(PARAM_DISCHARGE.chars().all(|c| c.is_ascii_digit()));
        assert!(PARAM_STAGE.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(PARAM_DISCHARGE, PARAM_STAGE);
    }

    #[test]
    fn test_all_stations_have_at_least_one_expected_parameter() {
        for station in STATION_REGISTRY {
            assert!(
                !station.expected_parameters.is_empty(),
                "station '{}' must have at least one expected parameter",
                station.name
            );
        }
    }

    #[test]
    fn test_sites_with_parameter_filters_correctly() {
        let discharge_sites = sites_with_parameter(PARAM_DISCHARGE);
        let stage_sites = sites_with_parameter(PARAM_STAGE);
        
        // All sites should have discharge
        assert_eq!(discharge_sites.len(), 8);
        
        // Chicago Canal likely doesn't have stage
        assert!(stage_sites.len() >= 7);
        
        // Kingston Mines should have both
        assert!(discharge_sites.contains(&"05568500"));
        assert!(stage_sites.contains(&"05568500"));
    }

    #[test]
    fn test_station_has_parameter_helper() {
        assert!(station_has_parameter("05568500", PARAM_DISCHARGE));
        assert!(station_has_parameter("05568500", PARAM_STAGE));
        assert!(!station_has_parameter("00000000", PARAM_DISCHARGE)); // non-existent station
    }
}

// ---------------------------------------------------------------------------
// Integration Tests - Station API Verification
// ---------------------------------------------------------------------------
// 
// These tests verify that stations in the registry actually exist and return
// the expected parameters from the live USGS API. They are marked #[ignore]
// so they don't run during normal CI builds (which shouldn't depend on external
// API availability).
//
// To run these tests manually:
//   cargo test -- --ignored station_api
//
// These tests serve multiple purposes:
// 1. Verify station codes are correct and stations are active
// 2. Confirm expected parameters are actually available
// 3. Detect when USGS decommissions or reconfigures a station
// 4. Provide early warning of API changes

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Helper to make a real API request and check if a station returns data.
    /// Returns (site_exists, has_discharge, has_stage, error_message).
    #[allow(dead_code)]
    fn verify_station_api(site_code: &str) -> (bool, bool, bool, Option<String>) {
        use crate::ingest::usgs::{build_iv_url, parse_iv_response};
        
        // Request last hour of data for both parameters
        let url = build_iv_url(&[site_code], &[PARAM_DISCHARGE, PARAM_STAGE], "PT1H");
        
        let response = match reqwest::blocking::get(&url) {
            Ok(resp) => match resp.error_for_status() {
                Ok(r) => match r.text() {
                    Ok(text) => text,
                    Err(e) => return (false, false, false, Some(format!("Failed to read response: {}", e))),
                },
                Err(e) => return (false, false, false, Some(format!("HTTP error: {}", e))),
            },
            Err(e) => return (false, false, false, Some(format!("Request failed: {}", e))),
        };
        
        let readings = match parse_iv_response(&response) {
            Ok(r) => r,
            Err(e) => return (false, false, false, Some(format!("Parse error: {:?}", e))),
        };
        
        // Check if we got readings for this site
        let site_readings: Vec<_> = readings.iter().filter(|r| r.site_code == site_code).collect();
        
        if site_readings.is_empty() {
            return (false, false, false, Some("No readings returned for this site".to_string()));
        }
        
        let has_discharge = site_readings.iter().any(|r| r.parameter_code == PARAM_DISCHARGE);
        let has_stage = site_readings.iter().any(|r| r.parameter_code == PARAM_STAGE);
        
        (true, has_discharge, has_stage, None)
    }

    #[test]
    #[ignore] // Don't run in CI - depends on external API
    fn station_api_kingston_mines_returns_expected_data() {
        let (exists, has_discharge, has_stage, error) = verify_station_api("05568500");
        
        if let Some(err) = error {
            panic!("Station 05568500 (Kingston Mines) API check failed: {}", err);
        }
        
        assert!(exists, "Kingston Mines station should exist");
        assert!(has_discharge, "Kingston Mines should provide discharge (00060)");
        assert!(has_stage, "Kingston Mines should provide stage (00065)");
    }

    #[test]
    #[ignore] // Don't run in CI - depends on external API
    fn station_api_peoria_pool_returns_expected_data() {
        let (exists, has_discharge, has_stage, error) = verify_station_api("05567500");
        
        if let Some(err) = error {
            panic!("Station 05567500 (Peoria) API check failed: {}", err);
        }
        
        assert!(exists, "Peoria pool station should exist");
        assert!(has_discharge, "Peoria should provide discharge (00060)");
        assert!(has_stage, "Peoria should provide stage (00065)");
    }

    #[test]
    #[ignore] // Don't run in CI - depends on external API
    fn station_api_verify_all_registry_stations() {
        // This test verifies ALL stations in the registry
        let mut failures = Vec::new();
        let mut warnings = Vec::new();
        
        for station in STATION_REGISTRY {
            println!("\n🔍 Checking {} ({})...", station.name, station.site_code);
            
            let (exists, has_discharge, has_stage, error) = verify_station_api(station.site_code);
            
            if let Some(err) = error {
                failures.push(format!("{} ({}): {}", station.name, station.site_code, err));
                continue;
            }
            
            if !exists {
                failures.push(format!("{} ({}): Station does not exist or is offline", station.name, station.site_code));
                continue;
            }
            
            // Verify expected parameters match reality
            let expects_discharge = station.expected_parameters.contains(&PARAM_DISCHARGE);
            let expects_stage = station.expected_parameters.contains(&PARAM_STAGE);
            
            if expects_discharge && !has_discharge {
                warnings.push(format!("{} ({}): Expected discharge but not available", station.name, station.site_code));
            }
            
            if expects_stage && !has_stage {
                warnings.push(format!("{} ({}): Expected stage but not available", station.name, station.site_code));
            }
            
            if !expects_discharge && has_discharge {
                warnings.push(format!("{} ({}): Discharge available but not in expected_parameters", station.name, station.site_code));
            }
            
            if !expects_stage && has_stage {
                warnings.push(format!("{} ({}): Stage available but not in expected_parameters", station.name, station.site_code));
            }
            
            println!("   ✓ exists={}, discharge={}, stage={}", exists, has_discharge, has_stage);
        }
        
        // Print summary
        if !warnings.is_empty() {
            println!("\n⚠️  WARNINGS ({}):", warnings.len());
            for warning in &warnings {
                println!("   - {}", warning);
            }
        }
        
        if !failures.is_empty() {
            println!("\n❌ FAILURES ({}):", failures.len());
            for failure in &failures {
                println!("   - {}", failure);
            }
            panic!("Station API verification failed for {} station(s)", failures.len());
        }
        
        if warnings.is_empty() {
            println!("\n✅ All {} stations verified successfully!", STATION_REGISTRY.len());
        } else {
            println!("\n⚠️  {} stations verified with {} warnings", STATION_REGISTRY.len(), warnings.len());
        }
    }

    #[test]
    #[ignore] // Don't run in CI - depends on external API
    fn station_api_invalid_site_returns_no_data() {
        // Verify that a made-up station code returns no data
        let (exists, _, _, _) = verify_station_api("99999999");
        assert!(!exists, "Fake station should not return data");
    }
}

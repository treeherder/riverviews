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
    },
];

/// Returns the site codes for all monitored stations as a `Vec<&str>`,
/// suitable for passing directly to `ingest::usgs::build_iv_url`.
pub fn all_site_codes() -> Vec<&'static str> {
    STATION_REGISTRY.iter().map(|s| s.site_code).collect()
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
}

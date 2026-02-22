/// Station configuration loader - parses usgs_stations.toml
///
/// Separates station metadata from code, making it easy to update
/// thresholds, add stations, or adjust travel time estimates without
/// recompiling the service.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::model::FloodThresholds;

/// Station metadata loaded from usgs_stations.toml configuration file
#[derive(Debug, Clone, Deserialize)]
pub struct StationConfig {
    pub site_code: String,
    pub name: String,
    pub description: String,
    
    // Geographic location
    pub latitude: f64,
    pub longitude: f64,
    
    // Positioning relative to Peoria (for lead time calculations)
    pub distance_from_peoria_miles: f64,
    pub distance_direction: String,  // "upstream", "downstream", "tributary_south", etc.
    pub travel_time_to_peoria_hours: f64,
    
    // NWS flood stage thresholds (optional - not all stations have official thresholds)
    pub thresholds: Option<ThresholdConfig>,
    
    // Expected USGS parameters at this site
    pub expected_parameters: Vec<String>,  // e.g., ["00060", "00065"]
    
    // Peak flow data metadata (optional)
    pub peak_flow: Option<PeakFlowMetadata>,
}

/// Flood stage thresholds from NWS AHPS
#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdConfig {
    pub action_stage_ft: f64,
    pub flood_stage_ft: f64,
    pub moderate_flood_stage_ft: f64,
    pub major_flood_stage_ft: f64,
    pub description: String,
}

/// Peak flow data availability and metadata
#[derive(Debug, Clone, Deserialize)]
pub struct PeakFlowMetadata {
    #[serde(default)]
    pub available: Option<bool>,  // false if no data in USGS database
    pub url: Option<String>,
    pub period_of_record: Option<String>,
    pub years_available: Option<u32>,
    pub notable_floods: Option<String>,
    pub notes: Option<String>,
}

/// Root configuration structure for TOML parsing
#[derive(Debug, Deserialize)]
struct StationRegistry {
    station: Vec<StationConfig>,
}

/// Loads station registry from usgs_stations.toml configuration file.
///
/// # Panics
/// Panics if the configuration file is missing, malformed, or contains
/// invalid data. This is intentional — the service cannot operate without
/// valid station metadata.
///
/// # File Location
/// Expects `usgs_stations.toml` in the current working directory (project root
/// when running via `cargo run`).
pub fn load_config() -> Vec<StationConfig> {
    let config_path = "usgs_stations.toml";
    
    let contents = fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", config_path, e));
    
    let registry: StationRegistry = toml::from_str(&contents)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", config_path, e));
    
    registry.station
}

/// Loads station registry and builds a lookup map keyed by site code.
///
/// Useful for O(1) station lookups by site code during data processing.
pub fn load_config_map() -> HashMap<String, StationConfig> {
    load_config()
        .into_iter()
        .map(|s| (s.site_code.clone(), s))
        .collect()
}

/// Converts ThresholdConfig from TOML to FloodThresholds model type.
///
/// This adapter function bridges the configuration layer and the domain model,
/// allowing the rest of the codebase to use the existing FloodThresholds type.
impl From<&ThresholdConfig> for FloodThresholds {
    fn from(config: &ThresholdConfig) -> Self {
        FloodThresholds {
            action_stage_ft: config.action_stage_ft,
            flood_stage_ft: config.flood_stage_ft,
            moderate_flood_stage_ft: config.moderate_flood_stage_ft,
            major_flood_stage_ft: config.major_flood_stage_ft,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_succeeds() {
        let stations = load_config();
        assert!(stations.len() >= 8, "Should have at least 8 stations");
    }

    #[test]
    fn test_all_stations_have_required_fields() {
        let stations = load_config();
        for station in stations {
            assert!(!station.site_code.is_empty(), "Site code must not be empty");
            assert!(!station.name.is_empty(), "Name must not be empty");
            assert!(station.latitude >= -90.0 && station.latitude <= 90.0);
            assert!(station.longitude >= -180.0 && station.longitude <= 180.0);
            assert!(station.expected_parameters.len() > 0, "Must have at least one parameter");
        }
    }

    #[test]
    fn test_kingston_mines_has_thresholds() {
        let stations = load_config();
        let kingston = stations.iter()
            .find(|s| s.site_code == "05568500")
            .expect("Kingston Mines should exist in config");
        
        assert!(kingston.thresholds.is_some(), "Kingston Mines should have flood thresholds");
        
        let thresholds = kingston.thresholds.as_ref().unwrap();
        assert_eq!(thresholds.action_stage_ft, 14.0);
        assert_eq!(thresholds.flood_stage_ft, 16.0);
        assert_eq!(thresholds.moderate_flood_stage_ft, 20.0);
        assert_eq!(thresholds.major_flood_stage_ft, 24.0);
    }

    #[test]
    fn test_peoria_pool_has_thresholds() {
        let stations = load_config();
        let peoria = stations.iter()
            .find(|s| s.site_code == "05567500")
            .expect("Peoria pool should exist in config");
        
        // Peoria pool is managed but still has flood stage thresholds
        let thresholds = peoria.thresholds.as_ref()
            .expect("Peoria pool should have flood thresholds");
        assert_eq!(thresholds.action_stage_ft, 17.0);
        assert_eq!(thresholds.flood_stage_ft, 18.0);
        assert_eq!(thresholds.moderate_flood_stage_ft, 20.0);
        assert_eq!(thresholds.major_flood_stage_ft, 22.0);
    }

    #[test]
    fn test_travel_times_reasonable() {
        let stations = load_config();
        for station in stations {
            // Travel times should be non-negative and under 100 hours
            assert!(station.travel_time_to_peoria_hours >= 0.0);
            assert!(station.travel_time_to_peoria_hours < 100.0);
        }
    }

    #[test]
    fn test_thresholds_ascending_order() {
        let stations = load_config();
        for station in stations {
            if let Some(t) = &station.thresholds {
                assert!(t.action_stage_ft < t.flood_stage_ft,
                    "{}: action must be < flood", station.name);
                assert!(t.flood_stage_ft < t.moderate_flood_stage_ft,
                    "{}: flood must be < moderate", station.name);
                assert!(t.moderate_flood_stage_ft < t.major_flood_stage_ft,
                    "{}: moderate must be < major", station.name);
            }
        }
    }

    #[test]
    fn test_config_map_lookup() {
        let map = load_config_map();
        assert!(map.contains_key("05568500"), "Should contain Kingston Mines");
        assert!(map.contains_key("05552500"), "Should contain Marseilles");
        
        let kingston = &map["05568500"];
        assert_eq!(kingston.name, "Illinois River at Kingston Mines, IL");
    }

    #[test]
    fn test_threshold_conversion() {
        let config = ThresholdConfig {
            action_stage_ft: 14.0,
            flood_stage_ft: 16.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 24.0,
            description: "Test thresholds".to_string(),
        };
        
        let thresholds: FloodThresholds = (&config).into();
        assert_eq!(thresholds.action_stage_ft, 14.0);
        assert_eq!(thresholds.flood_stage_ft, 16.0);
    }
}

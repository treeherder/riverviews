/// USACE/CWMS Location Registry
///
/// Loads USACE Corps Water Management System (CWMS) locations from configuration
/// file, providing metadata for lock/dam pool elevations, Mississippi River stages,
/// and other USACE-managed monitoring points.
///
/// ## Configuration File
/// 
/// Location metadata is loaded from `usace_iem.toml`, allowing updates to timeseries
/// IDs, relevance notes, and monitoring priorities without recompilation.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

// ---------------------------------------------------------------------------
// TOML Configuration Structures
// ---------------------------------------------------------------------------

/// Root configuration from usace_iem.toml
#[derive(Debug, Deserialize)]
struct UsaceConfig {
    #[serde(default)]
    usace_stations: Vec<UsaceStationConfig>,
    #[serde(default)]
    iem_asos_stations: Vec<IemAsosConfig>,
}

/// USACE station configuration from TOML
#[derive(Debug, Deserialize)]
struct UsaceStationConfig {
    shef_id: Option<String>,
    shef_pool_id: Option<String>,
    cwms_location: Option<String>,
    office: String,
    name: String,
    river_mile: Option<f64>,
    river_mile_above_ohio: Option<f64>,
    pool_elevation_target_ft_ngvd29: Option<f64>,
    datum_note: Option<String>,
    data_types: Vec<String>,
    relevance: String,
    flood_note: Option<String>,
}

/// IEM ASOS station configuration from TOML (for future use)
#[derive(Debug, Deserialize)]
struct IemAsosConfig {
    station_id: String,
    name: String,
    latitude: f64,
    longitude: f64,
    data_types: Vec<String>,
    relevance: String,
}

// ---------------------------------------------------------------------------
// Runtime Data Structures
// ---------------------------------------------------------------------------

/// Metadata for a single USACE/CWMS monitoring location
#[derive(Debug, Clone)]
pub struct UsaceLocation {
    /// SHEF ID (legacy identifier from rivergages.mvr.usace.army.mil)
    pub shef_id: Option<String>,
    
    /// CWMS location name (e.g., "Peoria-Pool", "Grafton")
    pub cwms_location: String,
    
    /// USACE district office (e.g., "MVR", "MVS")
    pub office: String,
    
    /// Human-readable location name
    pub name: String,
    
    /// River mile (Illinois River or Mississippi River)
    pub river_mile: Option<f64>,
    
    /// Pool elevation target (NGVD29 datum, for lock/dam pools)
    pub pool_target_ft: Option<f64>,
    
    /// Data types available (pool_elevation, tailwater_elevation, stage, discharge, etc.)
    pub data_types: Vec<String>,
    
    /// Relevance to flood monitoring (why we care about this location)
    pub relevance: String,
    
    /// Flood-specific operational notes
    pub flood_notes: Option<String>,
    
    /// Monitoring priority (derived from relevance)
    pub priority: MonitoringPriority,
    
    /// Discovered timeseries IDs (populated at runtime from CWMS catalog)
    pub discovered_timeseries: Option<DiscoveredTimeseries>,
}

/// Timeseries IDs discovered from CWMS catalog at runtime
#[derive(Debug, Clone)]
pub struct DiscoveredTimeseries {
    pub pool_elevation: Option<String>,
    pub tailwater_elevation: Option<String>,
    pub stage: Option<String>,
    pub discharge: Option<String>,
}

/// Monitoring priority for polling frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoringPriority {
    /// Poll every 15 minutes (critical real-time monitoring)
    Critical,
    /// Poll every hour (early warning)
    High,
    /// Poll every 6 hours (basin context)
    Medium,
    /// Poll daily (historical/analytical)
    Low,
}

// ---------------------------------------------------------------------------
// Configuration Loading
// ---------------------------------------------------------------------------

/// Load all USACE locations from configuration file
pub fn load_locations() -> Result<Vec<UsaceLocation>, String> {
    let config_path = "usace_iem.toml";
    
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read {}: {}", config_path, e))?;
    
    let config: UsaceConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", config_path, e))?;
    
    let locations = config.usace_stations
        .into_iter()
        .map(|station| {
            // Determine priority before moving relevance
            let priority = determine_priority(&station.relevance);
            
            UsaceLocation {
                shef_id: station.shef_id,
                cwms_location: station.cwms_location.unwrap_or_else(|| {
                    // If no cwms_location specified, derive from name
                    station.name.split(" at ").last()
                        .unwrap_or(&station.name)
                        .replace(", ", "-")
                        .replace(" ", "-")
                }),
                office: station.office,
                name: station.name,
                river_mile: station.river_mile.or(station.river_mile_above_ohio),
                pool_target_ft: station.pool_elevation_target_ft_ngvd29,
                data_types: station.data_types,
                relevance: station.relevance,
                flood_notes: station.flood_note,
                priority,
                discovered_timeseries: None, // Will be populated by discover_timeseries_ids()
            }
        })
        .collect();
    
    Ok(locations)
}

/// Load locations as a HashMap for O(1) lookups by CWMS location name
pub fn load_locations_map() -> Result<HashMap<String, UsaceLocation>, String> {
    let locations = load_locations()?;
    
    let map = locations.into_iter()
        .map(|loc| (loc.cwms_location.clone(), loc))
        .collect();
    
    Ok(map)
}

/// Find a specific location by CWMS location name
pub fn find_location(cwms_location: &str) -> Option<UsaceLocation> {
    load_locations()
        .ok()?
        .into_iter()
        .find(|loc| loc.cwms_location == cwms_location)
}

/// Determine monitoring priority from relevance text
fn determine_priority(relevance: &str) -> MonitoringPriority {
    let upper = relevance.to_uppercase();
    
    if upper.contains("PRIMARY") || upper.contains("CRITICAL") {
        MonitoringPriority::Critical
    } else if upper.contains("HIGH") || upper.contains("UPSTREAM WARNING") {
        MonitoringPriority::High
    } else if upper.contains("EXTENDED") || upper.contains("CONFLUENCE MONITOR") {
        MonitoringPriority::Medium
    } else {
        MonitoringPriority::Low
    }
}

/// Get poll interval in minutes for a monitoring priority
pub fn poll_interval_minutes(priority: MonitoringPriority) -> u64 {
    match priority {
        MonitoringPriority::Critical => 15,
        MonitoringPriority::High => 60,
        MonitoringPriority::Medium => 360,
        MonitoringPriority::Low => 1440, // daily
    }
}

/// Get all locations for a specific monitoring priority
pub fn locations_by_priority(priority: MonitoringPriority) -> Result<Vec<UsaceLocation>, String> {
    Ok(load_locations()?
        .into_iter()
        .filter(|loc| loc.priority == priority)
        .collect())
}

/// Discover actual CWMS timeseries IDs for a location using the catalog API
///
/// # Important
/// The TOML file contains provisional timeseries IDs based on documented patterns,
/// but the exact version suffix (CBT-RAW vs lrgs-rev vs Ccp-Rev) varies by office
/// and data stream. This function queries the CWMS catalog endpoint to discover
/// what timeseries are actually available.
///
/// # Example
/// ```no_run
/// # use flomon_service::usace_locations::{find_location, discover_timeseries_ids};
/// let location = find_location("Peoria-Pool").unwrap();
/// let client = reqwest::blocking::Client::new();
/// let discovered = discover_timeseries_ids(&client, &location)?;
/// // discovered.pool_elevation might be "Peoria-Pool.Elev.Inst.~1Hour.0.CBT-RAW"
/// # Ok::<(), String>(())
/// ```
pub fn discover_timeseries_ids(
    client: &reqwest::blocking::Client,
    location: &UsaceLocation,
) -> Result<DiscoveredTimeseries, String> {
    use crate::ingest::cwms;
    
    let data_types = &location.data_types;
    let mut discovered = DiscoveredTimeseries {
        pool_elevation: None,
        tailwater_elevation: None,
        stage: None,
        discharge: None,
    };
    
    // Discover pool elevation if needed
    if data_types.contains(&"pool_elevation".to_string()) {
        discovered.pool_elevation = cwms::discover_pool_elevation(
            client,
            &location.office,
            &location.cwms_location
        ).map_err(|e| format!("Failed to discover pool elevation: {}", e))?;
        
        if let Some(ref ts_id) = discovered.pool_elevation {
            println!("      Discovered pool elevation: {}", ts_id);
        }
    }
    
    // Discover tailwater elevation if needed
    if data_types.contains(&"tailwater_elevation".to_string()) {
        discovered.tailwater_elevation = cwms::discover_tailwater_elevation(
            client,
            &location.office,
            &location.cwms_location
        ).map_err(|e| format!("Failed to discover tailwater elevation: {}", e))?;
        
        if let Some(ref ts_id) = discovered.tailwater_elevation {
            println!("      Discovered tailwater elevation: {}", ts_id);
        }
    }
    
    // Discover stage if needed (for river gauges, not pools)
    if data_types.contains(&"stage".to_string()) {
        discovered.stage = cwms::discover_stage(
            client,
            &location.office,
            &location.cwms_location
        ).map_err(|e| format!("Failed to discover stage: {}", e))?;
        
        if let Some(ref ts_id) = discovered.stage {
            println!("      Discovered stage: {}", ts_id);
        }
    }
    
    Ok(discovered)
}

/// Update a location with discovered timeseries IDs
pub fn update_with_discovered_timeseries(
    location: &mut UsaceLocation,
    client: &reqwest::blocking::Client,
) -> Result<(), String> {
    let discovered = discover_timeseries_ids(client, location)?;
    
    // Check if we found at least one timeseries
    if discovered.pool_elevation.is_none() 
        && discovered.tailwater_elevation.is_none() 
        && discovered.stage.is_none() 
        && discovered.discharge.is_none() {
        return Err(format!("No timeseries found for location: {}", location.name));
    }
    
    location.discovered_timeseries = Some(discovered);
    Ok(())
}

// ---------------------------------------------------------------------------
// CWMS Timeseries ID Construction
// ---------------------------------------------------------------------------

/// Build CWMS timeseries ID for a location
///
/// # Format
/// ```text
/// {LOCATION}.{PARAM}.{TYPE}.{INTERVAL}.{DURATION}.{VERSION}
/// ```
///
/// # Example
/// ```text
/// Peoria-Pool.Elev.Inst.~1Hour.0.CBT-RAW
/// ```
///
/// # Note
/// The exact suffix (version/source like "CBT-RAW" vs "lrgs-rev") must be
/// verified against the CWMS catalog endpoint for each office.
pub fn build_timeseries_id(
    location: &str,
    parameter: &str,
    type_: &str,
    interval: &str,
    version: &str,
) -> String {
    format!("{}.{}.{}.{}.0.{}", location, parameter, type_, interval, version)
}

/// Build pool elevation timeseries ID (common pattern)
pub fn build_pool_elev_id(location: &str) -> String {
    build_timeseries_id(location, "Elev", "Inst", "~1Hour", "CBT-RAW")
}

/// Build tailwater elevation timeseries ID (common pattern)
pub fn build_tailwater_elev_id(location: &str) -> String {
    build_timeseries_id(
        &format!("{}-TW", location.trim_end_matches("-Pool")),
        "Elev",
        "Inst",
        "~1Hour",
        "CBT-RAW"
    )
}

/// Build stage timeseries ID (for river gauges, not pools)
pub fn build_stage_id(location: &str) -> String {
    build_timeseries_id(location, "Stage", "Inst", "15Minutes", "Ccp-Rev")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_locations() {
        let locations = load_locations().expect("Failed to load locations");
        
        // Should have at least Peoria, LaGrange, and Grafton
        assert!(locations.len() >= 3, "Expected at least 3 locations");
        
        // Find Peoria
        let peoria = locations.iter()
            .find(|loc| loc.cwms_location.contains("Peoria"))
            .expect("Peoria location not found");
        
        assert_eq!(peoria.office, "MVR");
        assert!(peoria.pool_target_ft.is_some());
        assert!(peoria.priority == MonitoringPriority::Critical);
    }
    
    #[test]
    fn test_priority_determination() {
        assert_eq!(determine_priority("PRIMARY — this is critical"), MonitoringPriority::Critical);
        assert_eq!(determine_priority("HIGH UPSTREAM WARNING"), MonitoringPriority::High);
        assert_eq!(determine_priority("EXTENDED lead time"), MonitoringPriority::Medium);
    }
    
    #[test]
    fn test_timeseries_id_construction() {
        let id = build_pool_elev_id("Peoria-Pool");
        assert_eq!(id, "Peoria-Pool.Elev.Inst.~1Hour.0.CBT-RAW");
        
        let tw_id = build_tailwater_elev_id("Peoria-Pool");
        assert_eq!(tw_id, "Peoria-TW.Elev.Inst.~1Hour.0.CBT-RAW");
    }
}

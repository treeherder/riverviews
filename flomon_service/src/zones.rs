/// Zone-based sensor grouping from zones.toml
///
/// Organizes sensors into hydrologically meaningful geographic zones
/// with lead times and flood forecasting context.

use serde::Deserialize;
use std::fs;
use std::path::Path;

// ============================================================================
// TOML Configuration Structures
// ============================================================================

/// Root zones configuration
#[derive(Debug, Deserialize)]
pub struct ZonesConfig {
    pub zones: ZoneCollection,
}

/// Collection of all zones
#[derive(Debug, Deserialize)]
pub struct ZoneCollection {
    pub zone_0: Zone,
    pub zone_1: Zone,
    pub zone_2: Zone,
    pub zone_3: Zone,
    pub zone_4: Zone,
    pub zone_5: Zone,
    pub zone_6: Zone,
}

/// Single zone with sensors
#[derive(Debug, Deserialize, Clone)]
pub struct Zone {
    pub name: String,
    pub description: String,
    pub sensors: Vec<Sensor>,
}

/// Individual sensor within a zone
#[derive(Debug, Deserialize, Clone)]
pub struct Sensor {
    #[serde(rename = "id")]
    pub sensor_id: Option<String>,         // SHEF ID or custom ID
    pub usgs_id: Option<String>,           // USGS site code
    pub station_id: Option<String>,        // ASOS station ID
    pub cwms_location: Option<String>,     // CWMS location ID
    pub shef_id: Option<String>,           // SHEF ID (legacy)
    pub source: String,                    // "USGS", "USACE/MVR", "USACE/MVS", "IEM/ASOS", etc.
    #[serde(rename = "type")]
    pub sensor_type: String,               // "stage", "discharge", "pool_elevation", "precipitation", etc.
    pub role: String,                      // "direct", "boundary", "precip", "proxy"
    pub location: String,                  // Human-readable location description
    pub lat: f64,
    pub lon: f64,
    pub relevance: String,                 // Why this sensor matters for this zone
    
    // Optional metadata
    pub pool_target_ft_ngvd29: Option<f64>,
    pub flood_stage_ft: Option<f64>,
    pub action_stage_ft: Option<f64>,
    pub moderate_flood_ft: Option<f64>,
    pub major_flood_ft: Option<f64>,
    pub datum_note: Option<String>,
}

// ============================================================================
// Zone Metadata
// ============================================================================

/// Zone metadata with lead times and alert thresholds
#[derive(Debug, Clone)]
pub struct ZoneMetadata {
    pub zone_id: usize,
    pub name: String,
    pub lead_time_hours_min: Option<i64>,
    pub lead_time_hours_max: Option<i64>,
    pub primary_alert_condition: String,
}

impl ZoneMetadata {
    /// Get zone metadata for a specific zone
    pub fn for_zone(zone_id: usize) -> Self {
        match zone_id {
            0 => ZoneMetadata {
                zone_id: 0,
                name: "Mississippi River — Backwater Source".to_string(),
                lead_time_hours_min: Some(12),
                lead_time_hours_max: Some(120), // 5 days
                primary_alert_condition: "Grafton stage > 20 ft".to_string(),
            },
            1 => ZoneMetadata {
                zone_id: 1,
                name: "Lower Illinois River — Backwater Interface".to_string(),
                lead_time_hours_min: Some(6),
                lead_time_hours_max: Some(24),
                primary_alert_condition: "LaGrange TW → pool diff < 1 ft".to_string(),
            },
            2 => ZoneMetadata {
                zone_id: 2,
                name: "Upper Peoria Lake — Property Zone (Primary)".to_string(),
                lead_time_hours_min: Some(0),
                lead_time_hours_max: Some(6),
                primary_alert_condition: "Peoria pool > 447.5 ft / Kingston Mines stage > 14 ft".to_string(),
            },
            3 => ZoneMetadata {
                zone_id: 3,
                name: "Local Tributaries — Mackinaw and Spoon Catchments".to_string(),
                lead_time_hours_min: Some(6),
                lead_time_hours_max: Some(18),
                primary_alert_condition: "Mackinaw rate-of-rise > 1 ft/hr".to_string(),
            },
            4 => ZoneMetadata {
                zone_id: 4,
                name: "Mid Illinois River — Starved Rock to Henry".to_string(),
                lead_time_hours_min: Some(18),
                lead_time_hours_max: Some(48),
                primary_alert_condition: "Henry stage > 15 ft".to_string(),
            },
            5 => ZoneMetadata {
                zone_id: 5,
                name: "Upper Illinois River — Confluence to Starved Rock".to_string(),
                lead_time_hours_min: Some(36),
                lead_time_hours_max: Some(72),
                primary_alert_condition: "Dresden pool elevated + Kankakee rising".to_string(),
            },
            6 => ZoneMetadata {
                zone_id: 6,
                name: "Chicago CAWS — Lake Michigan Inflow and MWRD Control".to_string(),
                lead_time_hours_min: Some(72),
                lead_time_hours_max: Some(120), // 3-5 days
                primary_alert_condition: "O'Hare 6hr precip > 1.5 in + CSSC discharge spike".to_string(),
            },
            _ => ZoneMetadata {
                zone_id,
                name: format!("Unknown Zone {}", zone_id),
                lead_time_hours_min: None,
                lead_time_hours_max: None,
                primary_alert_condition: "N/A".to_string(),
            },
        }
    }
}

// ============================================================================
// Loading Functions
// ============================================================================

/// Load zones configuration from TOML file
pub fn load_zones<P: AsRef<Path>>(path: P) -> Result<ZonesConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: ZonesConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Load zones from default location (zones.toml)
pub fn load_zones_default() -> Result<ZonesConfig, Box<dyn std::error::Error>> {
    load_zones("zones.toml")
}

/// Get all zones as a vector (in order 0-6)
pub fn get_all_zones(config: &ZonesConfig) -> Vec<(usize, &Zone)> {
    vec![
        (0, &config.zones.zone_0),
        (1, &config.zones.zone_1),
        (2, &config.zones.zone_2),
        (3, &config.zones.zone_3),
        (4, &config.zones.zone_4),
        (5, &config.zones.zone_5),
        (6, &config.zones.zone_6),
    ]
}

/// Get a specific zone by ID
pub fn get_zone<'a>(config: &'a ZonesConfig, zone_id: usize) -> Option<&'a Zone> {
    match zone_id {
        0 => Some(&config.zones.zone_0),
        1 => Some(&config.zones.zone_1),
        2 => Some(&config.zones.zone_2),
        3 => Some(&config.zones.zone_3),
        4 => Some(&config.zones.zone_4),
        5 => Some(&config.zones.zone_5),
        6 => Some(&config.zones.zone_6),
        _ => None,
    }
}

// ============================================================================
// Sensor Lookup Helpers
// ============================================================================

impl Sensor {
    /// Get the primary identifier for this sensor (prioritize USGS, then CWMS, then ASOS, then custom)
    pub fn primary_id(&self) -> String {
        self.usgs_id.clone()
            .or_else(|| self.cwms_location.clone())
            .or_else(|| self.station_id.clone())
            .or_else(|| self.sensor_id.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string())
    }
    
    /// Check if this sensor is from USGS
    pub fn is_usgs(&self) -> bool {
        self.usgs_id.is_some()
    }
    
    /// Check if this sensor is from CWMS
    pub fn is_cwms(&self) -> bool {
        self.cwms_location.is_some() || self.source.contains("USACE")
    }
    
    /// Check if this sensor is from ASOS
    pub fn is_asos(&self) -> bool {
        self.station_id.is_some() && self.source.contains("ASOS")
    }
    
    /// Get sensor role priority (for sorting)
    pub fn role_priority(&self) -> u8 {
        match self.role.as_str() {
            "direct" => 0,
            "boundary" => 1,
            "proxy" => 2,
            "precip" => 3,
            _ => 99,
        }
    }
}

impl Zone {
    /// Get sensors by role
    pub fn sensors_by_role(&self, role: &str) -> Vec<&Sensor> {
        self.sensors.iter()
            .filter(|s| s.role == role)
            .collect()
    }
    
    /// Get USGS sensors only
    pub fn usgs_sensors(&self) -> Vec<&Sensor> {
        self.sensors.iter()
            .filter(|s| s.is_usgs())
            .collect()
    }
    
    /// Get CWMS sensors only
    pub fn cwms_sensors(&self) -> Vec<&Sensor> {
        self.sensors.iter()
            .filter(|s| s.is_cwms())
            .collect()
    }
    
    /// Get ASOS sensors only
    pub fn asos_sensors(&self) -> Vec<&Sensor> {
        self.sensors.iter()
            .filter(|s| s.is_asos())
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_zone_metadata() {
        let zone2 = ZoneMetadata::for_zone(2);
        assert_eq!(zone2.zone_id, 2);
        assert!(zone2.name.contains("Peoria"));
        assert_eq!(zone2.lead_time_hours_min, Some(0));
    }
    
    #[test]
    fn test_sensor_primary_id() {
        let sensor = Sensor {
            sensor_id: Some("TEST".to_string()),
            usgs_id: Some("05568500".to_string()),
            station_id: None,
            cwms_location: None,
            shef_id: None,
            source: "USGS".to_string(),
            sensor_type: "stage".to_string(),
            role: "direct".to_string(),
            location: "Test".to_string(),
            lat: 40.0,
            lon: -89.0,
            relevance: "Test sensor".to_string(),
            pool_target_ft_ngvd29: None,
            flood_stage_ft: None,
            action_stage_ft: None,
            moderate_flood_ft: None,
            major_flood_ft: None,
            datum_note: None,
        };
        
        assert_eq!(sensor.primary_id(), "05568500");
        assert!(sensor.is_usgs());
        assert!(!sensor.is_cwms());
    }
}

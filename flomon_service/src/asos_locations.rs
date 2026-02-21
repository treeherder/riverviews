/// ASOS Station TOML Configuration Loader
///
/// Loads ASOS (Automated Surface Observing System) station metadata from
/// iem_asos.toml for weather monitoring relevant to tributary flood forecasting.

use serde::Deserialize;
use std::fs;
use std::path::Path;

// ============================================================================
// TOML Configuration Structures
// ============================================================================

/// Root TOML structure
#[derive(Debug, Deserialize)]
pub struct AsosConfig {
    pub stations: Vec<AsosStation>,
    pub iem_api: IemApiConfig,
}

/// Single ASOS station configuration
#[derive(Debug, Deserialize, Clone)]
pub struct AsosStation {
    pub station_id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: f64,
    pub data_types: Vec<String>,
    pub relevance: String,
    pub basin: String,
    pub upstream_gauge: String,
}

/// IEM API endpoint configuration
#[derive(Debug, Deserialize)]
pub struct IemApiConfig {
    pub current_url: String,
    pub asos_1min_url: String,
    pub daily_summary_url: String,
    pub iemre_url: String,
    pub mrms_url: String,
}

// ============================================================================
// Location Loading and Priority Detection
// ============================================================================

/// Monitoring priority based on relevance rating
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoringPriority {
    Critical,  // Poll every 15 minutes
    High,      // Poll every 60 minutes
    Medium,    // Poll every 6 hours
    Low,       // Poll daily
}

impl MonitoringPriority {
    pub fn poll_interval_minutes(&self) -> i64 {
        match self {
            MonitoringPriority::Critical => 15,
            MonitoringPriority::High => 60,
            MonitoringPriority::Medium => 360,
            MonitoringPriority::Low => 1440,
        }
    }
}

/// ASOS station with monitoring metadata
#[derive(Debug, Clone)]
pub struct AsosLocation {
    pub station_id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: f64,
    pub data_types: Vec<String>,
    pub relevance: String,
    pub basin: String,
    pub upstream_gauge: String,
    pub priority: MonitoringPriority,
}

/// Load ASOS stations from TOML file
pub fn load_locations<P: AsRef<Path>>(path: P) -> Result<Vec<AsosLocation>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: AsosConfig = toml::from_str(&content)?;
    
    let locations: Vec<AsosLocation> = config.stations.into_iter()
        .map(|station| {
            let priority = determine_priority(&station.relevance);
            
            AsosLocation {
                station_id: station.station_id,
                name: station.name,
                latitude: station.latitude,
                longitude: station.longitude,
                elevation_ft: station.elevation_ft,
                data_types: station.data_types,
                relevance: station.relevance,
                basin: station.basin,
                upstream_gauge: station.upstream_gauge,
                priority,
            }
        })
        .collect();
    
    Ok(locations)
}

/// Determine monitoring priority from relevance text
fn determine_priority(relevance: &str) -> MonitoringPriority {
    let lower = relevance.to_lowercase();
    
    if lower.contains("primary") || lower.contains("critical") {
        MonitoringPriority::Critical
    } else if lower.contains("high") || lower.contains("tributary") {
        MonitoringPriority::High
    } else if lower.contains("medium") || lower.contains("extended") {
        MonitoringPriority::Medium
    } else {
        MonitoringPriority::Low
    }
}

// ============================================================================
// Precipitation Thresholds by Basin
// ============================================================================

/// Precipitation thresholds for flood risk assessment
#[derive(Debug, Clone)]
pub struct PrecipThresholds {
    pub watch_6hr_in: f64,     // Issue watch
    pub warning_6hr_in: f64,   // Issue warning
    pub watch_24hr_in: f64,
    pub warning_24hr_in: f64,
}

impl AsosLocation {
    /// Get precipitation thresholds for this basin
    pub fn precip_thresholds(&self) -> PrecipThresholds {
        match self.basin.as_str() {
            "Mackinaw River" => PrecipThresholds {
                watch_6hr_in: 1.0,
                warning_6hr_in: 2.0,
                watch_24hr_in: 2.5,
                warning_24hr_in: 4.0,
            },
            "Spoon River" => PrecipThresholds {
                watch_6hr_in: 1.2,
                warning_6hr_in: 2.5,
                watch_24hr_in: 3.0,
                warning_24hr_in: 5.0,
            },
            "Sangamon River" => PrecipThresholds {
                watch_6hr_in: 1.5,
                warning_6hr_in: 3.0,
                watch_24hr_in: 3.5,
                warning_24hr_in: 5.5,
            },
            "Des Plaines River" => PrecipThresholds {
                watch_6hr_in: 1.0,
                warning_6hr_in: 2.0,
                watch_24hr_in: 2.5,
                warning_24hr_in: 4.5,
            },
            "Illinois River" | _ => PrecipThresholds {
                watch_6hr_in: 1.5,
                warning_6hr_in: 2.5,
                watch_24hr_in: 3.0,
                warning_24hr_in: 5.0,
            },
        }
    }
    
    /// Get lag time (hours) from precipitation to stream response
    pub fn tributary_lag_hours(&self) -> i64 {
        match self.basin.as_str() {
            "Mackinaw River" => 12,    // Bloomington to Green Valley
            "Spoon River" => 18,       // Galesburg to Seville
            "Sangamon River" => 24,    // Springfield to Oakford
            "Des Plaines River" => 6,  // Chicago to Joliet (fast response)
            "Illinois River" | _ => 48, // Mainstem (slow response)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_priority_detection() {
        assert_eq!(
            determine_priority("Primary local precip station"),
            MonitoringPriority::Critical
        );
        
        assert_eq!(
            determine_priority("High-value tributary monitoring"),
            MonitoringPriority::High
        );
        
        assert_eq!(
            determine_priority("Extended coverage for Sangamon basin"),
            MonitoringPriority::Medium
        );
    }
    
    #[test]
    fn test_precip_thresholds() {
        let location = AsosLocation {
            station_id: "KPIA".to_string(),
            name: "Peoria".to_string(),
            latitude: 40.664,
            longitude: -89.693,
            elevation_ft: 660.0,
            data_types: vec!["precipitation".to_string()],
            relevance: "Primary".to_string(),
            basin: "Illinois River".to_string(),
            upstream_gauge: "05568500".to_string(),
            priority: MonitoringPriority::Critical,
        };
        
        let thresholds = location.precip_thresholds();
        assert_eq!(thresholds.watch_6hr_in, 1.5);
        assert_eq!(thresholds.warning_24hr_in, 5.0);
    }
    
    #[test]
    fn test_tributary_lag() {
        let mackinaw = AsosLocation {
            station_id: "KBMI".to_string(),
            name: "Bloomington".to_string(),
            latitude: 40.477,
            longitude: -88.916,
            elevation_ft: 871.0,
            data_types: vec!["precipitation".to_string()],
            relevance: "High".to_string(),
            basin: "Mackinaw River".to_string(),
            upstream_gauge: "05568000".to_string(),
            priority: MonitoringPriority::High,
        };
        
        assert_eq!(mackinaw.tributary_lag_hours(), 12);
    }
}

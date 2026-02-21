/// Core daemon implementation for flood monitoring service
///
/// This module implements the main daemon loop that:
/// 1. Validates database connectivity and schemas on startup
/// 2. Detects staleness of existing data
/// 3. Backfills missing historical data
/// 4. Continuously polls USGS/USACE APIs for new data
/// 5. Warehouses readings and maintains monitoring state
/// 6. Generates alerts for threshold exceedances and staleness

use crate::db;
use crate::stations::{self, Station};
use crate::model::GaugeReading;
use chrono::{DateTime, Duration, Utc};
use postgres::Client;
use std::collections::HashMap;
use std::error::Error;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Daemon configuration
pub struct DaemonConfig {
    /// How often to poll USGS API (default: 15 minutes to match USGS update frequency)
    pub poll_interval_minutes: u64,
    
    /// Maximum age of data before considered stale (default: 60 minutes)
    pub staleness_threshold_minutes: u64,
    
    /// How many days of historical data to backfill (default: 120 days)
    pub backfill_days: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            poll_interval_minutes: 15,
            staleness_threshold_minutes: 60,
            backfill_days: 120,
        }
    }
}

// ---------------------------------------------------------------------------
// Daemon State
// ---------------------------------------------------------------------------

/// Main daemon state
pub struct Daemon {
    config: DaemonConfig,
    stations: Vec<Station>,
    client: Option<Client>,
}

impl Daemon {
    /// Create a new daemon instance with default configuration
    pub fn new() -> Self {
        Self {
            config: DaemonConfig::default(),
            stations: Vec::new(),
            client: None,
        }
    }
    
    /// Create daemon with custom configuration
    pub fn with_config(config: DaemonConfig) -> Self {
        Self {
            config,
            stations: Vec::new(),
            client: None,
        }
    }
    
    /// Initialize daemon: validate database and load stations
    pub fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        // Validate database schemas
        let client = db::connect_and_verify(&["usgs_raw", "nws", "usace"])?;
        self.client = Some(client);
        
        // Load station registry
        self.stations = stations::load_stations();
        
        if self.stations.is_empty() {
            return Err("No stations configured in stations.toml".into());
        }
        
        Ok(())
    }
    
    /// Check staleness of data for a specific station
    pub fn check_staleness(&mut self, site_code: &str) -> Result<Option<Duration>, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let rows = client.query(
            "SELECT MAX(reading_time) as latest 
             FROM usgs_raw.gauge_readings 
             WHERE site_code = $1",
            &[&site_code]
        )?;
        
        if rows.is_empty() {
            // No data found - this is maximum staleness
            return Ok(None);
        }
        
        let latest: Option<DateTime<Utc>> = rows[0].get(0);
        
        match latest {
            Some(dt) => Ok(Some(Utc::now() - dt)),
            None => Ok(None), // No readings in database
        }
    }
    
    /// Check if backfill is needed for a station
    pub fn needs_backfill(&mut self, site_code: &str) -> Result<bool, Box<dyn Error>> {
        match self.check_staleness(site_code)? {
            None => Ok(true), // No data at all
            Some(staleness) => {
                // Need backfill if data is older than threshold
                Ok(staleness.num_minutes() > self.config.staleness_threshold_minutes as i64)
            }
        }
    }
    
    /// Backfill historical data for a station
    pub fn backfill_station(&mut self, site_code: &str) -> Result<usize, Box<dyn Error>> {
        // TODO: Implement backfill logic
        // 1. Determine date range to fetch (based on latest data or backfill_days config)
        // 2. Call USGS IV API for historical data
        // 3. Parse response and insert readings
        // 4. Return count of inserted readings
        
        let _ = site_code;
        unimplemented!("backfill_station: fetch and insert historical data")
    }
    
    /// Poll a single station for latest data
    pub fn poll_station(&mut self, site_code: &str) -> Result<Vec<GaugeReading>, Box<dyn Error>> {
        // TODO: Implement polling logic
        // 1. Build USGS IV API URL for last 4 hours
        // 2. Fetch data
        // 3. Parse response
        // 4. Return readings (caller will warehouse them)
        
        let _ = site_code;
        unimplemented!("poll_station: fetch latest readings from USGS API")
    }
    
    /// Warehouse readings into database (idempotent)
    pub fn warehouse_readings(&mut self, readings: &[GaugeReading]) -> Result<usize, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let mut inserted = 0;
        
        for reading in readings {
            // Use INSERT ... ON CONFLICT DO NOTHING for idempotency
            let rows_affected = client.execute(
                "INSERT INTO usgs_raw.gauge_readings 
                 (site_code, parameter_code, unit, value, reading_time, qualifier)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING",
                &[
                    &reading.site_code,
                    &reading.parameter_code,
                    &reading.unit,
                    &reading.value,
                    &reading.datetime,
                    &reading.qualifier,
                ]
            )?;
            
            inserted += rows_affected as usize;
        }
        
        Ok(inserted)
    }
    
    /// Update monitoring state after successful poll
    pub fn update_monitoring_state(
        &mut self, 
        site_code: &str, 
        last_reading_time: Option<DateTime<Utc>>
    ) -> Result<(), Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        // Update or insert monitoring state
        client.execute(
            "INSERT INTO usgs_raw.monitoring_state 
             (site_code, parameter_code, last_poll_attempted, latest_reading_time, consecutive_failures)
             VALUES ($1, '00060', $2, $3, 0)
             ON CONFLICT (site_code) DO UPDATE SET
                last_poll_attempted = EXCLUDED.last_poll_attempted,
                latest_reading_time = EXCLUDED.latest_reading_time,
                consecutive_failures = 0",
            &[&site_code, &Utc::now(), &last_reading_time]
        )?;
        
        Ok(())
    }
    
    /// Record a polling failure
    pub fn record_failure(&mut self, site_code: &str) -> Result<(), Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        client.execute(
            "INSERT INTO usgs_raw.monitoring_state 
             (site_code, parameter_code, last_poll_attempted, consecutive_failures)
             VALUES ($1, '00060', $2, 1)
             ON CONFLICT (site_code) DO UPDATE SET
                last_poll_attempted = EXCLUDED.last_poll_attempted,
                consecutive_failures = monitoring_state.consecutive_failures + 1",
            &[&site_code, &Utc::now()]
        )?;
        
        Ok(())
    }
    
    /// Run one iteration of the monitoring loop for all stations
    pub fn poll_all_stations(&mut self) -> Result<HashMap<String, usize>, Box<dyn Error>> {
        let mut results = HashMap::new();
        
        for station in &self.stations.clone() {
            match self.poll_station(&station.site_code) {
                Ok(readings) => {
                    let inserted = self.warehouse_readings(&readings)?;
                    
                    // Get latest timestamp from readings
                    let latest = readings.iter()
                        .filter_map(|r| chrono::DateTime::parse_from_rfc3339(&r.datetime).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .max();
                    
                    self.update_monitoring_state(&station.site_code, latest)?;
                    results.insert(station.site_code.clone(), inserted);
                }
                Err(e) => {
                    eprintln!("Failed to poll {}: {}", station.site_code, e);
                    self.record_failure(&station.site_code)?;
                    results.insert(station.site_code.clone(), 0);
                }
            }
        }
        
        Ok(results)
    }
    
    /// Main daemon loop (runs indefinitely)
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("🚀 Starting daemon loop...");
        println!("   Poll interval: {} minutes", self.config.poll_interval_minutes);
        println!("   Monitoring {} stations", self.stations.len());
        
        loop {
            let start = Utc::now();
            
            match self.poll_all_stations() {
                Ok(results) => {
                    let total: usize = results.values().sum();
                    println!("✓ Poll complete: {} new readings across {} stations", 
                            total, results.len());
                }
                Err(e) => {
                    eprintln!("✗ Poll error: {}", e);
                }
            }
            
            // Sleep until next poll interval
            let elapsed = (Utc::now() - start).num_seconds();
            let sleep_seconds = (self.config.poll_interval_minutes * 60) as i64 - elapsed;
            
            if sleep_seconds > 0 {
                std::thread::sleep(std::time::Duration::from_secs(sleep_seconds as u64));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_daemon_creation() {
        let daemon = Daemon::new();
        assert_eq!(daemon.config.poll_interval_minutes, 15);
        assert_eq!(daemon.config.staleness_threshold_minutes, 60);
        assert_eq!(daemon.config.backfill_days, 120);
    }
    
    #[test]
    fn test_custom_daemon_config() {
        let config = DaemonConfig {
            poll_interval_minutes: 5,
            staleness_threshold_minutes: 30,
            backfill_days: 30,
        };
        
        let daemon = Daemon::with_config(config);
        assert_eq!(daemon.config.poll_interval_minutes, 5);
        assert_eq!(daemon.config.staleness_threshold_minutes, 30);
        assert_eq!(daemon.config.backfill_days, 30);
    }
    
    #[test]
    fn test_daemon_requires_initialization() {
        let mut daemon = Daemon::new();
        
        // Should fail before initialization
        let result = daemon.check_staleness("05568500");
        assert!(result.is_err(), "Should fail before initialization");
    }
    
    // Additional tests would require database connection
    // See tests/daemon_lifecycle.rs for integration tests
}

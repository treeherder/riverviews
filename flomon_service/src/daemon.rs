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
use crate::logging;
use crate::stations::{self, Station};
use crate::usace_locations::{self, UsaceLocation};
use crate::asos_locations::{self, AsosLocation};
use crate::model::GaugeReading;
use crate::ingest::{usgs, cwms, iem};
use chrono::{DateTime, Duration, Utc};
use postgres::Client;
use rust_decimal::Decimal;
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
    cwms_locations: Vec<UsaceLocation>,
    asos_locations: Vec<AsosLocation>,
    client: Option<Client>,
}

impl Daemon {
    /// Create a new daemon instance with default configuration
    pub fn new() -> Self {
        Self {
            config: DaemonConfig::default(),
            stations: Vec::new(),
            cwms_locations: Vec::new(),
            asos_locations: Vec::new(),
            client: None,
        }
    }
    
    /// Create daemon with custom configuration
    pub fn with_config(config: DaemonConfig) -> Self {
        Self {
            config,
            stations: Vec::new(),
            cwms_locations: Vec::new(),
            asos_locations: Vec::new(),
            client: None,
        }
    }
    
    /// Initialize daemon: validate database and load stations
    pub fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        // Validate database schemas
        let client = db::connect_and_verify(&["usgs_raw", "nws", "usace"])?;
        
        // Load USGS station registry from TOML
        self.stations = stations::load_stations();
        
        if self.stations.is_empty() {
            return Err("No stations configured in usgs_stations.toml".into());
        }
        
        // Load CWMS locations from TOML
        let mut locations = usace_locations::load_locations()?;
        
        if locations.is_empty() {
            eprintln!("Warning: No USACE/CWMS locations configured in usace_iem.toml");
        } else {
            // Discover actual CWMS timeseries IDs from catalog endpoint
            println!("🔍 Discovering CWMS timeseries IDs from catalog...");
            let http_client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()?;
            
            for location in &mut locations {
                print!("   {} ... ", location.name);
                match usace_locations::update_with_discovered_timeseries(location, &http_client) {
                    Ok(_) => println!("✓"),
                    Err(e) => {
                        println!("✗ {}", e);
                        eprintln!("      Warning: Will skip polling for {}", location.name);
                    }
                }
            }
            
            // Filter to only locations with discovered timeseries
            let discovered_count = locations.iter()
                .filter(|loc| loc.discovered_timeseries.is_some())
                .count();
            
            println!("   Discovered timeseries for {}/{} locations\n", 
                    discovered_count, locations.len());
        }
        
        self.cwms_locations = locations;
        
        // Load ASOS locations from TOML
        let asos_path = std::path::Path::new("iem_asos.toml");
        if asos_path.exists() {
            let asos_locs = asos_locations::load_locations(asos_path)?;
            println!("📡 Loaded {} ASOS stations for precipitation monitoring", asos_locs.len());
            for loc in &asos_locs {
                println!("   {} ({}) - {} basin - Priority: {:?}",
                    loc.station_id, loc.name, loc.basin, loc.priority);
            }
            self.asos_locations = asos_locs;
        } else {
            eprintln!("Warning: iem_asos.toml not found, skipping ASOS monitoring");
        }
        
        self.client = Some(client);
        
        Ok(())
    }
    
    /// Get reference to loaded stations
    pub fn get_stations(&self) -> &[Station] {
        &self.stations
    }
    
    /// Get reference to loaded CWMS locations
    pub fn get_cwms_locations(&self) -> &[UsaceLocation] {
        &self.cwms_locations
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
    
    /// Check staleness of CWMS data for a specific location
    pub fn check_cwms_staleness(&mut self, location_id: &str) -> Result<Option<Duration>, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let rows = client.query(
            "SELECT MAX(timestamp) as latest 
             FROM usace.cwms_timeseries 
             WHERE location_id = $1",
            &[&location_id]
        )?;
        
        if rows.is_empty() {
            return Ok(None);
        }
        
        let latest: Option<DateTime<Utc>> = rows[0].get(0);
        
        match latest {
            Some(dt) => Ok(Some(Utc::now() - dt)),
            None => Ok(None),
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
    /// Uses intelligent strategy: IV API for recent data (high-res), DV API for deep history
    pub fn backfill_station(&mut self, site_code: &str) -> Result<usize, Box<dyn Error>> {
        let now = Utc::now();
        
        // Check what data we already have
        let latest_data = self.check_staleness(site_code)?;
        
        let mut total_inserted = 0;
        
        match latest_data {
            None => {
                // No data at all - get high-resolution recent data + optional deep history
                println!("   Empty database for {} - fetching high-resolution data", site_code);
                
                // Always get the last 120 days as instantaneous values (high resolution)
                match self.backfill_instantaneous_values(site_code, 120) {
                    Ok(count) => {
                        total_inserted += count;
                        println!("   Fetched {} instantaneous readings (last 120 days)", count);
                    }
                    Err(e) => {
                        logging::log_usgs_failure(site_code, "IV backfill", &*e);
                        eprintln!("   Falling back to daily values for {}", site_code);
                        total_inserted += self.backfill_daily_values(
                            site_code, 
                            now - Duration::days(120), 
                            now
                        )?;
                    }
                }
                
                // Optionally get older data as daily values if backfill_days > 120
                if self.config.backfill_days > 120 {
                    let deep_history_days = self.config.backfill_days - 120;
                    println!("   Fetching {} additional days of daily values for historical context", deep_history_days);
                    
                    total_inserted += self.backfill_daily_values(
                        site_code,
                        now - Duration::days(self.config.backfill_days as i64),
                        now - Duration::days(120),
                    )?;
                }
            }
            Some(staleness) => {
                // We have some data - intelligently fill the gap
                let gap_days = staleness.num_days();
                
                if gap_days <= 120 {
                    // Gap is within IV API range - get high-resolution data
                    println!("   Filling {}-day gap with instantaneous values (high-res)", gap_days);
                    
                    match self.backfill_instantaneous_values(site_code, gap_days as u64) {
                        Ok(count) => {
                            total_inserted += count;
                            println!("   Fetched {} instantaneous readings", count);
                        }
                        Err(e) => {
                            logging::log_usgs_failure(site_code, "IV backfill (gap fill)", &*e);
                            eprintln!("   Falling back to daily values for {}", site_code);
                            total_inserted += self.backfill_daily_values(
                                site_code, 
                                now - staleness, 
                                now
                            )?;
                        }
                    }
                } else {
                    // Gap is too large for IV API - use hybrid strategy
                    println!("   Large gap ({} days) - using hybrid backfill", gap_days);
                    
                    // Get old data (beyond 120 days) as daily values
                    let old_data_start = now - staleness;
                    let old_data_end = now - Duration::days(120);
                    
                    if old_data_end > old_data_start {
                        let dv_count = self.backfill_daily_values(site_code, old_data_start, old_data_end)?;
                        total_inserted += dv_count;
                        println!("   Fetched {} daily values for days {}-120", dv_count, gap_days);
                    }
                    
                    // Get recent 120 days as instantaneous values (high resolution)
                    match self.backfill_instantaneous_values(site_code, 120) {
                        Ok(count) => {
                            total_inserted += count;
                            println!("   Fetched {} instantaneous readings (last 120 days)", count);
                        }
                        Err(e) => {
                            logging::log_usgs_failure(site_code, "Recent IV backfill", &*e);
                            eprintln!("   Falling back to daily values for {}", site_code);
                            total_inserted += self.backfill_daily_values(
                                site_code, 
                                now - Duration::days(120), 
                                now
                            )?;
                        }
                    }
                }
            }
        }
        
        Ok(total_inserted)
    }
    
    /// Backfill using Daily Values API (coarse resolution, longer history)
    fn backfill_daily_values(&mut self, site_code: &str, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<usize, Box<dyn Error>> {
        let start_date_str = start_date.format("%Y-%m-%d").to_string();
        let end_date_str = end_date.format("%Y-%m-%d").to_string();
        
        let url = usgs::build_dv_url(
            &[site_code],
            &["00060", "00065"], // Discharge and stage
            &start_date_str,
            &end_date_str,
        );
        
        println!("   Fetching daily values from {} to {}", start_date_str, end_date_str);
        
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        let response = client.get(&url).send()?;
        
        if !response.status().is_success() {
            return Err(format!("USGS API returned status {}", response.status()).into());
        }
        
        let body = response.text()?;
        
        let readings = match usgs::parse_dv_response(&body) {
            Ok(r) => r,
            Err(e) => {
                logging::log_usgs_failure(site_code, "DV API parsing", &e);
                return Ok(0);
            }
        };
        
        self.warehouse_readings(&readings)
    }
    
    /// Backfill using Instantaneous Values API (high resolution, limited history)
    fn backfill_instantaneous_values(&mut self, site_code: &str, days: u64) -> Result<usize, Box<dyn Error>> {
        // Convert days to ISO 8601 period format (e.g. P30D for 30 days)
        let period = format!("P{}D", days);
        
        let url = usgs::build_iv_url(
            &[site_code],
            &["00060", "00065"], // Discharge and stage
            &period,
        );
        
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        let response = client.get(&url).send()?;
        
        if !response.status().is_success() {
            return Err(format!("USGS API returned status {}", response.status()).into());
        }
        
        let body = response.text()?;
        
        // Use parse_iv_response_all to get ALL readings in the time period
        let readings = usgs::parse_iv_response_all(&body)?;
        
        self.warehouse_readings(&readings)
    }
    
    // ---------------------------------------------------------------------------
    // CWMS Data Acquisition
    // ---------------------------------------------------------------------------
    
    /// Poll a single CWMS location for latest data
    pub fn poll_cwms_location(&mut self, location: &UsaceLocation) -> Result<usize, Box<dyn Error>> {
        // Skip if no timeseries discovered
        let discovered = match &location.discovered_timeseries {
            Some(d) => d,
            None => return Ok(0), // No timeseries available, skip polling
        };
        
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        
        let mut total_inserted = 0;
        
        // Fetch pool elevation if available
        if let Some(ref ts_id) = discovered.pool_elevation {
            match cwms::fetch_recent(&http_client, ts_id, &location.office, 4) {
                Ok(timeseries) => {
                    total_inserted += self.warehouse_cwms_timeseries(&timeseries)?;
                }
                Err(e) => {
                    eprintln!("   Failed to fetch pool elevation for {}: {}", location.name, e);
                }
            }
        }
        
        // Fetch tailwater elevation if available
        if let Some(ref ts_id) = discovered.tailwater_elevation {
            match cwms::fetch_recent(&http_client, ts_id, &location.office, 4) {
                Ok(timeseries) => {
                    total_inserted += self.warehouse_cwms_timeseries(&timeseries)?;
                }
                Err(e) => {
                    eprintln!("   Failed to fetch tailwater elevation for {}: {}", location.name, e);
                }
            }
        }
        
        // Fetch stage if available (for river gauges)
        if let Some(ref ts_id) = discovered.stage {
            match cwms::fetch_recent(&http_client, ts_id, &location.office, 4) {
                Ok(timeseries) => {
                    total_inserted += self.warehouse_cwms_timeseries(&timeseries)?;
                }
                Err(e) => {
                    eprintln!("   Failed to fetch stage for {}: {}", location.name, e);
                }
            }
        }
        
        Ok(total_inserted)
    }
    
    /// Backfill CWMS location with historical data
    pub fn backfill_cwms_location(&mut self, location: &UsaceLocation) -> Result<usize, Box<dyn Error>> {
        // Skip if no timeseries discovered
        let discovered = match &location.discovered_timeseries {
            Some(d) => d,
            None => return Ok(0), // No timeseries available, skip backfill
        };
        
        let now = Utc::now();
        
        // Collect all timeseries IDs we need to backfill
        let mut timeseries_to_backfill = Vec::new();
        if let Some(ref ts_id) = discovered.pool_elevation {
            timeseries_to_backfill.push((ts_id.clone(), "pool"));
        }
        if let Some(ref ts_id) = discovered.tailwater_elevation {
            timeseries_to_backfill.push((ts_id.clone(), "tailwater"));
        }
        if let Some(ref ts_id) = discovered.stage {
            timeseries_to_backfill.push((ts_id.clone(), "stage"));
        }
        
        if timeseries_to_backfill.is_empty() {
            return Ok(0);
        }
        
        let mut total_inserted = 0;
        
        for (ts_id, param_type) in timeseries_to_backfill {
            // Check staleness for this specific timeseries
            let latest_data = self.check_cwms_staleness(&location.cwms_location)?;
            
            match latest_data {
                None => {
                    // No data at all - get last 120 days
                    println!("   Empty database for {} ({}) - fetching CWMS data", location.name, param_type);
                    
                    let http_client = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(30))
                        .build()?;
                    
                    let start = (now - Duration::days(120)).naive_utc();
                    let end = now.naive_utc();
                    
                    match cwms::fetch_historical(&http_client, &ts_id, &location.office, start, end) {
                        Ok(timeseries) => {
                            let inserted = self.warehouse_cwms_timeseries(&timeseries)?;
                            total_inserted += inserted;
                            println!("      Fetched {} {} readings", inserted, param_type);
                        }
                        Err(e) => {
                            eprintln!("      Failed to fetch {}: {}", param_type, e);
                        }
                    }
                }
                Some(staleness) => {
                    // We have some data - fill the gap if needed
                    let gap_days = staleness.num_days();
                    
                    if gap_days > 1 {
                        println!("   Filling {}-day CWMS gap for {} ({})", gap_days, location.name, param_type);
                        
                        let http_client = reqwest::blocking::Client::builder()
                            .timeout(std::time::Duration::from_secs(30))
                            .build()?;
                        
                        let start = (now - staleness).naive_utc();
                        let end = now.naive_utc();
                        
                        match cwms::fetch_historical(&http_client, &ts_id, &location.office, start, end) {
                            Ok(timeseries) => {
                                let inserted = self.warehouse_cwms_timeseries(&timeseries)?;
                                total_inserted += inserted;
                                println!("      Fetched {} {} readings", inserted, param_type);
                            }
                            Err(e) => {
                                eprintln!("      Failed to fetch {}: {}", param_type, e);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(total_inserted)
    }
    
    /// Warehouse CWMS timeseries into database (idempotent)
    fn warehouse_cwms_timeseries(&mut self, timeseries: &[cwms::CwmsTimeseries]) -> Result<usize, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let mut inserted = 0;
        
        for record in timeseries {
            // Convert value to Decimal for PostgreSQL NUMERIC type
            let value_decimal = rust_decimal::Decimal::from_f64_retain(record.value)
                .ok_or_else(|| format!("Failed to convert value {} to decimal", record.value))?;
            
            // Use INSERT ... ON CONFLICT DO NOTHING for idempotency
            let rows_affected = client.execute(
                "INSERT INTO usace.cwms_timeseries 
                 (location_id, timeseries_id, parameter_id, parameter_type, interval, duration, version,
                  timestamp, value, unit, quality_code)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                 ON CONFLICT (location_id, timestamp, parameter_id) DO NOTHING",
                &[
                    &record.location_id,
                    &record.timeseries_id,
                    &record.parameter_id,
                    &"Inst",  // parameter_type - instantaneous
                    &"15Minutes",  // interval
                    &"0",  // duration
                    &"Ccp-Rev",  // version
                    &record.timestamp,
                    &value_decimal,
                    &record.unit,
                    &record.quality_code,
                ]
            )?;
            
            inserted += rows_affected as usize;
        }
        
        Ok(inserted)
    }
    
    // ---------------------------------------------------------------------------
    // ASOS Weather Data Warehousing
    // ---------------------------------------------------------------------------
    
    /// Warehouse ASOS observations into database (idempotent)
    fn warehouse_asos_observations(&mut self, observations: &[iem::AsosObservation]) -> Result<usize, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let mut inserted = 0;
        
        for obs in observations {
            // Determine data source
            let data_source = "IEM_ASOS";
            
            let rows_affected = client.execute(
                "INSERT INTO asos_observations 
                 (station_id, observation_time, temp_f, dewpoint_f, relative_humidity,
                  wind_direction_deg, wind_speed_knots, wind_gust_knots, precip_1hr_in,
                  pressure_mb, visibility_mi, sky_condition, weather_codes, data_source)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                 ON CONFLICT (station_id, observation_time) DO NOTHING",
                &[
                    &obs.station_id,
                    &obs.timestamp,
                    &obs.temp_f,
                    &obs.dewpoint_f,
                    &obs.relative_humidity,
                    &obs.wind_direction_deg,
                    &obs.wind_speed_knots,
                    &obs.wind_gust_knots,
                    &obs.precip_1hr_in,
                    &obs.pressure_mb,
                    &obs.visibility_mi,
                    &obs.sky_condition,
                    &obs.weather_codes,
                    &data_source,
                ]
            )?;
            
            inserted += rows_affected as usize;
        }
        
        Ok(inserted)
    }
    
    /// Poll ASOS station for recent observations
    fn poll_asos_station(&mut self, station_id: &str) -> Result<Vec<iem::AsosObservation>, Box<dyn Error>> {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        
        // Fetch last 4 hours for recent poll
        let observations = iem::fetch_recent_precip(&http_client, station_id, 4)?;
        
        Ok(observations)
    }
    
    /// Backfill ASOS historical data for a station
    fn backfill_asos_station(&mut self, station_id: &str, days: i64) -> Result<usize, Box<dyn Error>> {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        let hours = days * 24;
        let observations = iem::fetch_recent_precip(&http_client, station_id, hours)?;
        
        self.warehouse_asos_observations(&observations)
    }
    
    // ---------------------------------------------------------------------------
    // USGS Data Warehousing
    // ---------------------------------------------------------------------------
    
    /// Poll a single station for latest data
    pub fn poll_station(&mut self, site_code: &str) -> Result<Vec<GaugeReading>, Box<dyn Error>> {
        // Build URL for instantaneous values (last 4 hours to ensure we get recent data)
        // USGS updates IV data every 15-60 minutes depending on the station
        let url = usgs::build_iv_url(
            &[site_code],
            &["00060", "00065"], // Discharge and stage
            "PT4H", // Last 4 hours
        );
        
        // Fetch data from USGS API
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        
        let response = client.get(&url).send()?;
        
        if !response.status().is_success() {
            return Err(format!("USGS API returned status {}", response.status()).into());
        }
        
        let body = response.text()?;
        
        // Parse response - note this returns the most recent value per parameter
        let readings = usgs::parse_iv_response(&body)?;
        
        Ok(readings)
    }
    
    /// Warehouse readings into database (idempotent)
    pub fn warehouse_readings(&mut self, readings: &[GaugeReading]) -> Result<usize, Box<dyn Error>> {
        let client = self.client.as_mut()
            .ok_or("Daemon not initialized")?;
        
        let mut inserted = 0;
        
        for reading in readings {
            // Parse datetime string to DateTime<Utc>
            // Try RFC3339 first (for instantaneous values with timezone)
            let reading_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&reading.datetime) {
                dt.with_timezone(&Utc)
            } else {
                // Fall back to NaiveDateTime for daily values (no timezone)
                // Assume local time is UTC for daily values
                let naive = chrono::NaiveDateTime::parse_from_str(&reading.datetime, "%Y-%m-%dT%H:%M:%S%.3f")
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(&reading.datetime, "%Y-%m-%d"))
                    .map_err(|e| format!("Failed to parse datetime '{}': {}", reading.datetime, e))?;
                chrono::DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
            };
            
            // Convert value to Decimal for PostgreSQL NUMERIC type
            let value_decimal = rust_decimal::Decimal::from_f64_retain(reading.value)
                .ok_or_else(|| format!("Failed to convert value {} to decimal", reading.value))?;
            
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
                    &value_decimal,
                    &reading_time,
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
        
        // Poll USGS stations
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
                    results.insert(format!("USGS:{}", station.site_code), inserted);
                }
                Err(e) => {
                    eprintln!("Failed to poll USGS {}: {}", station.site_code, e);
                    self.record_failure(&station.site_code)?;
                    results.insert(format!("USGS:{}", station.site_code), 0);
                }
            }
        }
        
        // Poll CWMS locations
        for location in &self.cwms_locations.clone() {
            match self.poll_cwms_location(&location) {
                Ok(inserted) => {
                    results.insert(format!("CWMS:{}", location.name), inserted);
                }
                Err(e) => {
                    eprintln!("Failed to poll CWMS {}: {}", location.name, e);
                    results.insert(format!("CWMS:{}", location.name), 0);
                }
            }
        }
        
        // Poll ASOS stations (based on priority)
        for location in &self.asos_locations.clone() {
            match self.poll_asos_station(&location.station_id) {
                Ok(observations) => {
                    let inserted = self.warehouse_asos_observations(&observations)?;
                    results.insert(format!("ASOS:{}", location.station_id), inserted);
                }
                Err(e) => {
                    eprintln!("Failed to poll ASOS {}: {}", location.station_id, e);
                    results.insert(format!("ASOS:{}", location.station_id), 0);
                }
            }
        }
        
        Ok(results)
    }
    
    /// Main daemon loop (runs indefinitely)
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("🚀 Starting daemon loop...");
        println!("   Poll interval: {} minutes", self.config.poll_interval_minutes);
        println!("   Monitoring {} USGS stations + {} CWMS locations + {} ASOS stations", 
                self.stations.len(), self.cwms_locations.len(), self.asos_locations.len());
        
        loop {
            let start = Utc::now();
            
            match self.poll_all_stations() {
                Ok(results) => {
                    let total: usize = results.values().sum();
                    let usgs_count = results.iter().filter(|(k, _)| k.starts_with("USGS:")).count();
                    let cwms_count = results.iter().filter(|(k, _)| k.starts_with("CWMS:")).count();
                    let asos_count = results.iter().filter(|(k, _)| k.starts_with("ASOS:")).count();
                    println!("✓ Poll complete: {} new readings ({} USGS, {} CWMS, {} ASOS)", 
                            total, usgs_count, cwms_count, asos_count);
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

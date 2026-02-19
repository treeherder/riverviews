#!/usr/bin/env rust
//! Historical data ingestion for USGS gauge readings.
//!
//! This binary performs initial database population and periodic backfills
//! using a two-tier approach:
//! 1. Daily Values (DV) API: Historical data from 1939 to ~125 days ago
//! 2. Instantaneous Values (IV) API: Recent high-resolution data (<120 days)
//!
//! It maintains state in a file to track:
//! - Whether initial DV historical population is complete
//! - Whether initial IV recent data population is complete
//! - Last successful update timestamp
//! - Progress through historical DV ingestion (by year)
//!
//! This dual approach provides both long-term context for flood modeling
//! and high-resolution recent data for operational monitoring.

use flomon_service::ingest::usgs::{build_dv_url, build_iv_url, parse_dv_response, parse_iv_response};
use flomon_service::model::GaugeReading;
use flomon_service::stations::{all_site_codes, PARAM_DISCHARGE, PARAM_STAGE};

use chrono::{DateTime, Datelike, Duration, Utc};
use postgres::{Client, NoTls};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATE_FILE: &str = "historical_ingest_state.json";

// USGS data availability
const DV_EARLIEST_YEAR: i32 = 1939;  // Site 05568500 has data from Oct 1939
const IV_LOOKBACK_DAYS: i64 = 120;   // IV API limit

// ---------------------------------------------------------------------------
// State Management
// ---------------------------------------------------------------------------

/// Persistent state tracking historical data ingestion progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IngestState {
    /// True if initial DV historical backfill is complete.
    dv_initialized: bool,
    
    /// True if initial IV recent data backfill is complete.
    iv_initialized: bool,
    
    /// ISO 8601 timestamp of last successful update.
    /// Example: "2026-02-19T06:00:00Z"
    last_update: Option<String>,
    
    /// Last year successfully ingested for DV data (e.g., 2015).
    /// Used to resume DV ingestion if interrupted.
    #[serde(default)]
    dv_last_year_completed: Option<i32>,
    
    /// Per-site ingestion tracking (future: resume partial failures).
    #[serde(default)]
    site_progress: std::collections::HashMap<String, String>,
}

impl Default for IngestState {
    fn default() -> Self {
        Self {
            dv_initialized: false,
            iv_initialized: false,
            last_update: None,
            dv_last_year_completed: None,
            site_progress: std::collections::HashMap::new(),
        }
    }
}

impl IngestState {
    /// Load state from disk, creating default if missing.
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = PathBuf::from(STATE_FILE);
        
        if !path.exists() {
            println!("📋 No state file found, creating new state");
            return Ok(Self::default());
        }
        
        let contents = fs::read_to_string(&path)?;
        let state: IngestState = serde_json::from_str(&contents)?;
        
        println!("📋 Loaded state:");
        println!("   DV initialized: {}", state.dv_initialized);
        println!("   IV initialized: {}", state.iv_initialized);
        println!("   Last DV year: {:?}", state.dv_last_year_completed);
        println!("   Last update: {:?}", state.last_update);
        
        Ok(state)
    }
    
    /// Save state to disk.
    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(STATE_FILE, json)?;
        println!("💾 Saved state to {}", STATE_FILE);
        Ok(())
    }
    
    /// Mark DV ingestion as complete for a given year.
    fn mark_dv_year_complete(&mut self, year: i32) {
        self.dv_last_year_completed = Some(year);
    }
    
    /// Mark full initialization as complete and update timestamp.
    fn mark_fully_initialized(&mut self) {
        self.dv_initialized = true;
        self.iv_initialized = true;
        self.last_update = Some(Utc::now().to_rfc3339());
    }
    
    /// Update the last successful ingestion timestamp.
    fn update_timestamp(&mut self) {
        self.last_update = Some(Utc::now().to_rfc3339());
    }
}

// ---------------------------------------------------------------------------
// Data Fetching
// ---------------------------------------------------------------------------

/// Fetch Daily Values data for a specific date range.
fn fetch_dv_period(
    sites: &[&str],
    start_date: &str,
    end_date: &str,
) -> Result<Vec<GaugeReading>, Box<dyn std::error::Error>> {
    let url = build_dv_url(sites, &[PARAM_DISCHARGE, PARAM_STAGE], start_date, end_date);
    
    println!("🌐 Fetching DV: {} to {}", start_date, end_date);
    
    let response = reqwest::blocking::get(&url)?
        .error_for_status()?
        .text()?;
    
    let readings = parse_dv_response(&response)?;
    println!("   ✓ Received {} daily readings", readings.len());
    
    Ok(readings)
}

/// Fetch Instantaneous Values data for a specific ISO 8601 period.
fn fetch_iv_period(
    sites: &[&str],
    period: &str,
) -> Result<Vec<GaugeReading>, Box<dyn std::error::Error>> {
    let url = build_iv_url(sites, &[PARAM_DISCHARGE, PARAM_STAGE], period);
    
    println!("🌐 Fetching IV: period {}", period);
    
    let response = reqwest::blocking::get(&url)?
        .error_for_status()?
        .text()?;
    
    let readings = parse_iv_response(&response)?;
    println!("   ✓ Received {} instantaneous readings", readings.len());
    
    Ok(readings)
}

// ---------------------------------------------------------------------------
// Database Operations
// ---------------------------------------------------------------------------

/// Insert historical readings into the database.
///
/// Unlike real-time monitoring (which updates current values), this
/// stores all timestamped readings for historical analysis.
///
/// Returns statistics about which sites were successfully stored.
fn store_readings(
    client: &mut Client,
    readings: &[GaugeReading],
) -> Result<(), Box<dyn std::error::Error>> {
    if readings.is_empty() {
        println!("   ℹ️  No readings to store (all stations may be offline)");
        return Ok(());
    }
    
    // Collect site statistics for visibility
    let mut sites_seen = std::collections::HashSet::new();
    for reading in readings {
        sites_seen.insert(&reading.site_code);
    }
    
    println!("💾 Storing {} readings from {} stations...", 
             readings.len(), sites_seen.len());
    
    let mut transaction = client.transaction()?;
    
    let stmt = transaction.prepare(
        "INSERT INTO usgs_raw.gauge_readings \
         (site_code, parameter_code, reading_time, value, qualifiers) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING"
    )?;
    
    let mut inserted = 0;
    for reading in readings {
        let rows = transaction.execute(
            &stmt,
            &[
                &reading.site_code,
                &reading.parameter_code,
                &reading.datetime,
                &reading.value,
                &reading.qualifier,
            ],
        )?;
        inserted += rows;
    }
    
    transaction.commit()?;
    
    println!("   ✓ Inserted {} new readings ({} duplicates skipped)",
             inserted, readings.len() - inserted as usize);
    
    // Show which sites we got data from (helps identify offline stations)
    if sites_seen.len() < all_site_codes().len() {
        let mut site_list: Vec<_> = sites_seen.iter().cloned().collect();
        site_list.sort();
        println!("   📍 Active sites: {}", site_list.join(", "));
    }
    
    Ok(())
}

// ---------------------------------------------------------------------------
// Ingestion Logic
// ---------------------------------------------------------------------------

/// Perform initial historical DV backfill from 1939 to ~125 days ago.
fn ingest_historical_dv(
    client: &mut Client,
    state: &mut IngestState,
) -> Result<(), Box<dyn std::error::Error>> {
    let sites = all_site_codes();
    
    // Calculate the cutoff date (125 days ago, leaving room for IV data)
    let cutoff_date = Utc::now() - Duration::days(125);
    let end_year = cutoff_date.year();
    
    // Determine starting year (resume from last completed if applicable)
    let start_year = state.dv_last_year_completed
        .map(|y| y + 1)
        .unwrap_or(DV_EARLIEST_YEAR);
    
    if start_year > end_year {
        println!("✓ DV historical data already complete (1939-{})", end_year);
        state.dv_initialized = true;
        return Ok(());
    }
    
    println!("🔄 Starting DV historical backfill");
    println!("   Years: {} to {}", start_year, end_year);
    println!("   Sites: {}", sites.len());
    println!("   This will fetch ~{} years of daily data", end_year - start_year + 1);
    
    for year in start_year..=end_year {
        let year_start = format!("{}-01-01", year);
        let year_end = if year == end_year {
            // For the final year, use the cutoff date
            cutoff_date.format("%Y-%m-%d").to_string()
        } else {
            format!("{}-12-31", year)
        };
        
        println!("\n📅 Ingesting year {} ({} to {})", year, year_start, year_end);
        
        match fetch_dv_period(&sites, &year_start, &year_end) {
            Ok(readings) => {
                store_readings(client, &readings)?;
                state.mark_dv_year_complete(year);
                state.save()?;
                println!("   ✓ Year {} complete", year);
            }
            Err(e) => {
                eprintln!("   ✗ Error fetching year {}: {}", year, e);
                return Err(e);
            }
        }
        
        // Rate limiting: don't hammer the API
        if year < end_year {
            println!("   ⏸️  Sleeping 2 seconds (rate limit)...");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    
    println!("\n✅ DV historical backfill complete ({}-{})", start_year, end_year);
    state.dv_initialized = true;
    Ok(())
}

/// Perform initial IV backfill for recent high-resolution data (<120 days).
fn ingest_recent_iv(
    client: &mut Client,
    state: &mut IngestState,
) -> Result<(), Box<dyn std::error::Error>> {
    let sites = all_site_codes();
    
    println!("🔄 Starting IV recent data backfill");
    println!("   Period: Last {} days (15-minute resolution)", IV_LOOKBACK_DAYS);
    println!("   Sites: {}", sites.len());
    
    // IV API limit is P120D
    let max_period_days = 120;
    let mut days_remaining = IV_LOOKBACK_DAYS;
    
    while days_remaining > 0 {
        let period_days = days_remaining.min(max_period_days);
        let period = format!("P{}D", period_days);
        
        println!("\n📦 Fetching {} days of IV data...", period_days);
        
        match fetch_iv_period(&sites, &period) {
            Ok(readings) => {
                store_readings(client, &readings)?;
                println!("   ✓ Successfully fetched and stored data");
            }
            Err(e) => {
                eprintln!("   ✗ Error fetching data: {}", e);
                return Err(e);
            }
        }
        
        days_remaining -= period_days;
        
        // Rate limiting
        if days_remaining > 0 {
            println!("   ⏸️  Sleeping 2 seconds (rate limit)...");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    
    println!("\n✅ IV recent data backfill complete!");
    state.iv_initialized = true;
    Ok(())
}

/// Perform incremental update since last run (IV data only for recent updates).
fn incremental_update(
    client: &mut Client,
    state: &IngestState,
) -> Result<(), Box<dyn std::error::Error>> {
    let last_update = state.last_update.as_ref()
        .ok_or("No last_update timestamp in state")?;
    
    let last_time = DateTime::parse_from_rfc3339(last_update)?;
    let now = Utc::now();
    let duration = now.signed_duration_since(last_time);
    
    println!("🔄 Performing incremental update (IV data only)");
    println!("   Last update: {}", last_update);
    println!("   Time elapsed: {} hours", duration.num_hours());
    
    // Determine period to fetch (IV only - DV is historical and doesn't need frequent updates)
    let period = if duration.num_days() > 0 {
        format!("P{}D", duration.num_days().min(120))
    } else {
        format!("PT{}H", duration.num_hours().max(1))
    };
    
    let sites = all_site_codes();
    
    println!("📦 Fetching IV period: {}", period);
    match fetch_iv_period(&sites, &period) {
        Ok(readings) => {
            store_readings(client, &readings)?;
            println!("✅ Incremental update complete: {} readings", readings.len());
        }
        Err(e) => {
            eprintln!("✗ Error during incremental update: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
}

// ---------------------------------------------------------------------------
// Main Entry Point
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 FloPro Historical Data Ingestion");
    println!("====================================");
    println!("Two-tier strategy:");
    println!("  1. Daily Values (DV): 1939 to ~125 days ago");
    println!("  2. Instantaneous Values (IV): Last 120 days (15-min resolution)\n");
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            eprintln!("❌ ERROR: DATABASE_URL environment variable not set");
            eprintln!("   Create a .env file with: DATABASE_URL=postgresql://user:pass@localhost/dbname");
            std::process::exit(1);
        });
    
    println!("🔌 Connecting to database...");
    let mut client = Client::connect(&database_url, NoTls)?;
    println!("   ✓ Connected\n");
    
    // Load state
    let mut state = IngestState::load()?;
    
    // Determine what to do
    if !state.dv_initialized || !state.iv_initialized {
        println!("ℹ️  Database not fully initialized - performing initial backfill\n");
        
        // Step 1: Ingest historical DV data if not complete
        if !state.dv_initialized {
            println!("━━━ PHASE 1: Historical Daily Values (1939-present) ━━━\n");
            ingest_historical_dv(&mut client, &mut state)?;
            state.save()?;
            println!("\n");
        }
        
        // Step 2: Ingest recent IV data if not complete
        if !state.iv_initialized {
            println!("━━━ PHASE 2: Recent Instantaneous Values (120 days) ━━━\n");
            ingest_recent_iv(&mut client, &mut state)?;
            state.save()?;
        }
        
        state.mark_fully_initialized();
        state.save()?;
        
    } else {
        println!("ℹ️  Database already initialized - performing incremental update\n");
        
        incremental_update(&mut client, &state)?;
        
        state.update_timestamp();
        state.save()?;
    }
    
    println!("\n🎉 Ingestion complete!");
    println!("   Next run will fetch data since: {}", 
             state.last_update.as_ref().unwrap());
    
    Ok(())
}

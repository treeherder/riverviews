#!/usr/bin/env rust
//! Peak Flow Event Ingestion
//!
//! Fetches USGS Peak Streamflow database records (annual peak stage/discharge)
//! and populates nws.flood_events table with historical flood events.
//!
//! For each station with defined flood thresholds:
//! 1. Fetch RDB format peak flow data from USGS
//! 2. Parse tab-delimited records (skip # comment lines)
//! 3. Identify floods: any peak where gage_ht >= flood_stage_ft
//! 4. Classify severity: flood (minor), moderate, or major
//! 5. Insert into nws.flood_events table
//!
//! Usage:
//!   cargo run --bin ingest_peak_flows
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string (from .env)

use flomon_service::config::{load_config, StationConfig};
use flomon_service::ingest::peak_flow::{
    parse_rdb, identify_flood_events, FloodThresholds, FloodEvent,
};

use chrono::{Duration, TimeZone, Utc};
use postgres::Client;
use rust_decimal::Decimal;

const USGS_PEAK_BASE_URL: &str = "https://nwis.waterdata.usgs.gov";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 Peak Flow Event Ingestion");
    println!("================================\n");
    
    // Connect to database with validation
    println!("📊 Connecting to database...");
    let mut client = flomon_service::db::connect_and_verify(&["usgs_raw", "nws"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", e);
            eprintln!("\nRun setup validation: ./scripts/validate_db_setup.sh\n");
            std::process::exit(1);
        });
    println!("✓ Connected\n");
    
    // Load station configuration
    println!("📋 Loading station configuration...");
    let stations = load_config();
    println!("✓ Loaded {} stations\n", stations.len());
    
    // Process each station that has thresholds defined
    let mut total_events = 0;
    let mut processed_stations = 0;
    let mut skipped_stations = 0;
    
    for station in &stations {
        match process_station(&mut client, station) {
            Ok(count) => {
                total_events += count;
                processed_stations += 1;
            }
            Err(e) => {
                eprintln!("⚠ Skipped {}: {}", station.site_code, e);
                skipped_stations += 1;
            }
        }
    }
    
    println!("\n🎉 INGESTION COMPLETE");
    println!("================================");
    println!("Stations processed: {}", processed_stations);
    println!("Stations skipped:   {}", skipped_stations);
    println!("Total flood events: {}", total_events);
    
    Ok(())
}

fn process_station(
    client: &mut Client,
    station: &StationConfig,
) -> Result<usize, Box<dyn std::error::Error>> {
    
    // Skip stations without thresholds
    let thresholds = match &station.thresholds {
        Some(t) => t,
        None => {
            return Err(format!("No flood thresholds defined").into());
        }
    };
    
    println!("📍 Processing: {} ({})", station.name, station.site_code);
    
    // Construct USGS Peak Streamflow URL
    // Format: https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=XXXXXXXX&agency_cd=USGS&format=rdb
    let url = format!(
        "{}/il/nwis/peak?site_no={}&agency_cd=USGS&format=rdb",
        USGS_PEAK_BASE_URL,
        station.site_code
    );
    
    println!("   Fetching: {}", url);
    
    // Fetch RDB data
    let response = reqwest::blocking::get(&url)?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()).into());
    }
    
    let rdb_text = response.text()?;
    
    // Check for "No sites/data found" message
    if rdb_text.contains("No sites/data found") {
        return Err("No peak flow data available".into());
    }
    
    // Parse RDB format
    let records = parse_rdb(&rdb_text)?;
    println!("   ✓ Parsed {} annual peak records", records.len());
    
    if records.is_empty() {
        return Ok(0);
    }
    
    // Identify flood events
    let flood_thresholds = FloodThresholds {
        flood_stage_ft: thresholds.flood_stage_ft,
        moderate_flood_stage_ft: thresholds.moderate_flood_stage_ft,
        major_flood_stage_ft: thresholds.major_flood_stage_ft,
    };
    
    let events = identify_flood_events(&records, &flood_thresholds);
    println!("   🌊 Identified {} flood events", events.len());
    
    if events.is_empty() {
        return Ok(0);
    }
    
    // Insert into database
    let inserted = insert_flood_events(client, &events, station)?;
    println!("   ✓ Inserted {} events into database\n", inserted);
    
    Ok(inserted)
}

fn insert_flood_events(
    client: &mut Client,
    events: &[FloodEvent],
    station: &StationConfig,
) -> Result<usize, Box<dyn std::error::Error>> {
    
    let mut inserted = 0;
    
    // Begin transaction
    let mut tx = client.transaction()?;
    
    for event in events {
        // Convert NaiveDateTime to timezone-aware DateTime<Utc>
        let crest_utc = Utc.from_utc_datetime(&event.crest_time);
        let event_start = crest_utc - Duration::hours(24);
        
        // Check if event already exists (avoid duplicates on re-run)
        let exists: i64 = tx.query_one(
            "SELECT COUNT(*) FROM nws.flood_events 
             WHERE site_code = $1 AND crest_time = $2",
            &[&station.site_code, &crest_utc],
        )?.get(0);
        
        if exists > 0 {
            continue; // Skip duplicate
        }
        
        // Insert flood event
        tx.execute(
            "INSERT INTO nws.flood_events 
             (site_code, event_start, event_end, crest_time, peak_stage_ft, severity, data_source)
             VALUES ($1, $2, NULL, $3, $4, $5, $6)",
            &[
                &station.site_code,
                &event_start,
                &crest_utc,
                &Decimal::from_f64_retain(event.peak_stage_ft).unwrap(),
                &event.severity.as_str(),
                &"USGS Peak Streamflow Database",
            ],
        )?;
        
        inserted += 1;
    }
    
    // Commit transaction
    tx.commit()?;
    
    Ok(inserted)
}

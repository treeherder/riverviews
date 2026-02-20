#!/usr/bin/env rust
//! CWMS Historical Data Ingestion
//!
//! Fetches historical timeseries data from USACE CWMS Data API for:
//! 1. Mississippi River stages (backwater flood detection)
//! 2. Illinois River lock/dam operations
//!
//! Data available since ~2015 when CWMS Data Dissemination went public.
//!
//! Usage:
//!   cargo run --bin ingest_cwms_historical
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string (from .env)
//!
//! Note: CWMS API has rate limits. This ingests data in daily chunks
//! to avoid overwhelming the API.

use flomon_service::ingest::cwms;
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use postgres::Client;
use rust_decimal::Decimal;
use std::thread;
use std::time;

// Start of CWMS public data availability
const CWMS_DATA_START: &str = "2015-01-01";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 CWMS Historical Data Ingestion");
    println!("==================================\n");
    
    // Connect to database with validation
    println!("📊 Connecting to database...");
    let mut client = flomon_service::db::connect_and_verify(&["usace"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", e);
            eprintln!("\nRun setup validation: ./scripts/validate_db_setup.sh\n");
            std::process::exit(1);
        });
    println!("✓ Connected\n");
    
    // HTTP client with timeout
    let http_client = reqwest::blocking::Client::builder()
        .timeout(time::Duration::from_secs(30))
        .build()?;
    
    // Load locations to ingest
    println!("📋 Loading CWMS locations to ingest...");
    let locations = load_monitored_locations(&mut client)?;
    println!("✓ Found {} monitored locations\n", locations.len());
    
    // Determine ingestion period
    let start_date = NaiveDate::parse_from_str(CWMS_DATA_START, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .ok_or("Invalid start time")?;
    let end_date = Utc::now().naive_utc();
    
    println!("📅 Ingestion period: {} to {}\n", 
             start_date.format("%Y-%m-%d"), 
             end_date.format("%Y-%m-%d"));
    
    let mut total_ingested = 0;
    let mut total_skipped = 0;
    let mut failed_locations = Vec::new();
    
    // Process each location
    for (idx, location) in locations.iter().enumerate() {
        println!("📍 [{}/{}] Processing: {} ({})", 
                 idx + 1, locations.len(), location.location_name, location.location_id);
        
        match ingest_location_data(
            &http_client,
            &mut client,
            location,
            start_date,
            end_date,
        ) {
            Ok((ingested, skipped)) => {
                println!("   ✓ Inserted {} records ({} duplicates skipped)\n", ingested, skipped);
                total_ingested += ingested;
                total_skipped += skipped;
            }
            Err(e) => {
                println!("   ✗ Error: {}\n", e);
                failed_locations.push(location.location_id.clone());
            }
        }
        
        // Rate limiting: sleep 2 seconds between requests
        if idx < locations.len() - 1 {
            thread::sleep(time::Duration::from_secs(2));
        }
    }
    
    println!("\n🎉 INGESTION COMPLETE");
    println!("==================================");
    println!("Locations processed: {}", locations.len());
    println!("Records inserted:    {}", total_ingested);
    println!("Duplicates skipped:  {}", total_skipped);
    
    if !failed_locations.is_empty() {
        println!("\n⚠️  Failed locations ({}):", failed_locations.len());
        for loc in failed_locations {
            println!("   - {}", loc);
        }
    }
    
    Ok(())
}

#[derive(Debug)]
struct CwmsLocation {
    location_id: String,
    location_name: String,
    office_id: String,
}

fn load_monitored_locations(client: &mut Client) -> Result<Vec<CwmsLocation>, Box<dyn std::error::Error>> {
    let rows = client.query(
        "SELECT location_id, location_name, office_id 
         FROM usace.cwms_locations 
         WHERE monitored = true 
         ORDER BY location_id",
        &[],
    )?;
    
    let mut locations = Vec::new();
    for row in rows {
        locations.push(CwmsLocation {
            location_id: row.get(0),
            location_name: row.get(1),
            office_id: row.get(2),
        });
    }
    
    Ok(locations)
}

fn ingest_location_data(
    http_client: &reqwest::blocking::Client,
    db_client: &mut Client,
    location: &CwmsLocation,
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    
    // Fetch data from CWMS API
    println!("   Fetching from CWMS API...");
    let records = cwms::fetch_historical(
        http_client,
        &location.location_id,
        &location.office_id,
        start_date,
        end_date,
    )?;
    
    if records.is_empty() {
        return Ok((0, 0));
    }
    
    println!("   Retrieved {} records", records.len());
    
    // Insert into database
    let mut inserted = 0;
    let mut skipped = 0;
    
    let mut tx = db_client.transaction()?;
    
    for record in &records {
        // Check if record already exists
        let exists: i64 = tx.query_one(
            "SELECT COUNT(*) FROM usace.cwms_timeseries 
             WHERE timeseries_id = $1 AND timestamp = $2",
            &[&record.timeseries_id, &record.timestamp],
        )?.get(0);
        
        if exists > 0 {
            skipped += 1;
            continue;
        }
        
        // Insert record
        tx.execute(
            "INSERT INTO usace.cwms_timeseries 
             (location_id, timeseries_id, parameter_id, parameter_type, interval, 
              timestamp, value, unit, quality_code, data_source)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &record.location_id,
                &record.timeseries_id,
                &record.parameter_id,
                &"Inst",  // Assuming instantaneous for now
                &"15Minutes",  // Default interval
                &record.timestamp,
                &Decimal::from_f64_retain(record.value).ok_or("Invalid decimal")?,
                &record.unit,
                &record.quality_code,
                &"CWMS_API_HISTORICAL",
            ],
        )?;
        
        inserted += 1;
    }
    
    tx.commit()?;
    
    // Log ingestion
    db_client.execute(
        "INSERT INTO usace.cwms_ingestion_log 
         (location_id, timeseries_id, query_start, query_end, 
          records_retrieved, records_inserted, records_skipped, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        &[
            &location.location_id,
            &location.location_id,
            &Utc.from_utc_datetime(&start_date),
            &Utc.from_utc_datetime(&end_date),
            &(records.len() as i32),
            &(inserted as i32),
            &(skipped as i32),
            &"success",
        ],
    )?;
    
    Ok((inserted, skipped))
}

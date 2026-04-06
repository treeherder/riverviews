//! Flood Monitoring Service - Main Daemon
//!
//! A server-side daemon that continuously:
//! 1. Ingests data from USGS, USACE, and NWS sources
//! 2. Validates and curates data in PostgreSQL
//! 3. Monitors for threshold exceedances and data staleness
//! 4. Provides HTTP endpoint for querying site data
//! 5. Maintains reliable data for external Python analysis
//!
//! Complex statistical analysis and regression modeling are handled
//! by external Python scripts that read from the curated database.
//!
//! Usage:
//!   cargo run --release -- verify          # Verify data source configuration
//!   cargo run --release -- --endpoint 8080 # Start daemon with HTTP endpoint on port 8080
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string

use flomon_service::daemon::Daemon;
use flomon_service::endpoint;
use flomon_service::logging::{self, LogLevel};
use std::env;

fn main() {
    println!("🌊 Flood Monitoring Service");
    println!("============================\n");
    
    // Parse command-line arguments early to check for verify command
    let args: Vec<String> = env::args().collect();
    
    // Check for verify command (runs without daemon initialization)
    if args.len() > 1 && args[1] == "verify" {
        println!("🔍 Running data source verification...\n");
        
        match flomon_service::verify::run_full_verification() {
            Ok(report) => {
                flomon_service::verify::print_summary(&report);
                
                // Save JSON report
                let report_json = serde_json::to_string_pretty(&report).unwrap();
                std::fs::write("verification_report.json", report_json).unwrap();
                println!("\n📄 Detailed report saved to: verification_report.json");
                
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("❌ Verification failed: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    // Initialize logging system
    // Log to both console and file in the current directory
    let log_file = "./flomon_service.log";
    let log_level = LogLevel::Info;  // Change to Debug for verbose output
    let console_timestamps = false;  // Clean console output, timestamps in file
    
    logging::init_logger(log_level, Some(log_file), console_timestamps);
    println!("📝 Logging to {}\n", log_file);
    
    // Parse remaining command-line arguments
    let mut endpoint_port: Option<u16> = None;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--endpoint" => {
                if i + 1 < args.len() {
                    endpoint_port = args[i + 1].parse().ok();
                    i += 2;
                } else {
                    eprintln!("Error: --endpoint requires a port number");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                eprintln!("Usage:");
                eprintln!("  {} verify           - Verify data source configuration", args[0]);
                eprintln!("  {} --endpoint PORT  - Start monitoring daemon", args[0]);
                std::process::exit(1);
            }
        }
    }
    
    // Create daemon with default configuration
    let mut daemon = Daemon::new();
    
    // Initialize: validate database and load stations
    println!("📊 Initializing daemon...");
    if let Err(e) = daemon.initialize() {
        eprintln!("\n❌ Initialization failed: {}\n", e);
        eprintln!("Run setup validation: ./scripts/validate_db_setup.sh\n");
        std::process::exit(1);
    }
    println!("✓ Daemon initialized\n");
    
    // Check for stale data and backfill if needed
    println!("📋 Checking data freshness...");
    let mut backfill_needed = Vec::new();
    
    // Collect station codes first to avoid borrow checker issues
    let station_codes: Vec<String> = daemon.get_stations()
        .iter()
        .map(|s| s.site_code.clone())
        .collect();
    
    for site_code in &station_codes {
        match daemon.check_staleness(site_code) {
            Ok(None) => {
                println!("   {} - No data found (needs backfill)", site_code);
                backfill_needed.push(site_code.clone());
            }
            Ok(Some(staleness)) => {
                let hours = staleness.num_hours();
                if hours > 2 {
                    println!("   {} - Data is {} hours old (stale)", site_code, hours);
                    backfill_needed.push(site_code.clone());
                } else {
                    println!("   {} - Data is fresh ({} min old)", site_code, staleness.num_minutes());
                }
            }
            Err(e) => {
                eprintln!("   {} - Error checking staleness: {}", site_code, e);
            }
        }
    }
    
    // Run backfill for stations that need it
    if !backfill_needed.is_empty() {
        println!("\n📥 Backfilling {} USGS stations...", backfill_needed.len());
        for site_code in &backfill_needed {
            match daemon.backfill_station(site_code) {
                Ok(count) => println!("   ✓ {} - Inserted {} readings", site_code, count),
                Err(e) => eprintln!("   ✗ {} - Backfill failed: {}", site_code, e),
            }
        }
        println!();
    }
    
    // Check CWMS locations for stale data
    println!("📋 Checking CWMS data freshness...");
    let mut cwms_backfill_needed = Vec::new();
    
    // Collect CWMS locations (clone to avoid borrow checker issues)
    let cwms_locations: Vec<_> = daemon.get_cwms_locations().to_vec();
    
    for location in &cwms_locations {
        // Skip locations without discovered timeseries
        if location.discovered_timeseries.is_none() {
            println!("   {} - Skipped (no timeseries discovered)", location.name);
            continue;
        }
        
        match daemon.check_cwms_staleness(&location.cwms_location) {
            Ok(None) => {
                println!("   {} - No data found (needs backfill)", location.name);
                cwms_backfill_needed.push(location.clone());
            }
            Ok(Some(staleness)) => {
                let hours = staleness.num_hours();
                if hours > 2 {
                    println!("   {} - Data is {} hours old (stale)", location.name, hours);
                    cwms_backfill_needed.push(location.clone());
                } else {
                    println!("   {} - Data is fresh ({} min old)", location.name, staleness.num_minutes());
                }
            }
            Err(e) => {
                eprintln!("   {} - Error checking staleness: {}", location.name, e);
            }
        }
    }
    
    // Run backfill for CWMS locations that need it
    if !cwms_backfill_needed.is_empty() {
        println!("\n📥 Backfilling {} CWMS locations...", cwms_backfill_needed.len());
        for location in &cwms_backfill_needed {
            match daemon.backfill_cwms_location(location) {
                Ok(count) => println!("   ✓ {} - Inserted {} readings", location.name, count),
                Err(e) => eprintln!("   ✗ {} - Backfill failed: {}", location.name, e),
            }
        }
        println!();
    }
    
    // Check ASOS stations for stale data
    println!("📋 Checking ASOS data freshness...");
    let asos_locations: Vec<_> = daemon.get_asos_locations().to_vec();
    let mut asos_backfill_needed = Vec::new();
    
    for location in &asos_locations {
        // Strip leading "K" to match IEM API station codes
        let station_id = if location.station_id.starts_with('K') && location.station_id.len() == 4 {
            &location.station_id[1..]
        } else {
            &location.station_id[..]
        };
        
        match daemon.check_asos_staleness(station_id) {
            Ok(None) => {
                println!("   {} - No data found (needs backfill)", location.station_id);
                asos_backfill_needed.push(station_id.to_string());
            }
            Ok(Some(staleness)) => {
                let hours = staleness.num_hours();
                if hours > 2 {
                    println!("   {} - Data is {} hours old (stale)", location.station_id, hours);
                    asos_backfill_needed.push(station_id.to_string());
                } else {
                    println!("   {} - Data is fresh ({} min old)", location.station_id, staleness.num_minutes());
                }
            }
            Err(e) => {
                eprintln!("   {} - Error checking staleness: {}", location.station_id, e);
            }
        }
    }
    
    if !asos_backfill_needed.is_empty() {
        println!("\n📥 Backfilling {} ASOS stations (last 30 days)...", asos_backfill_needed.len());
        for station_id in &asos_backfill_needed {
            match daemon.backfill_asos_station(station_id, 30) {
                Ok(count) => println!("   ✓ {} - Inserted {} observations", station_id, count),
                Err(e) => eprintln!("   ✗ {} - Backfill failed: {}", station_id, e),
            }
        }
        println!();
    }
    
    // Start HTTP endpoint if requested (in background thread)
    if let Some(port) = endpoint_port {
        println!("🚀 Starting HTTP endpoint server...");
        
        // Get a new database connection for the endpoint
        match flomon_service::db::connect_with_validation() {
            Ok(client) => {
                // Spawn endpoint server in background thread
                std::thread::spawn(move || {
                    if let Err(e) = endpoint::start_endpoint_server(port, client) {
                        eprintln!("❌ Endpoint server error: {}", e);
                    }
                });
                println!("   Endpoint running on http://0.0.0.0:{}\n", port);
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to database for endpoint: {}", e);
                eprintln!("   Continuing without HTTP endpoint\n");
            }
        }
    }
    
    // Run the main monitoring loop
    println!("🔄 Starting continuous monitoring loop...");
    println!("   Poll interval: 15 minutes");
    println!("   Monitoring {} USGS stations + {} CWMS locations", 
            daemon.get_stations().len(), daemon.get_cwms_locations().len());
    println!("   Press Ctrl+C to stop\n");
    
    if let Err(e) = daemon.run() {
        eprintln!("\n❌ Daemon error: {}", e);
        std::process::exit(1);
    }
}


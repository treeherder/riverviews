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
//!   cargo run --release                    # Start daemon without HTTP endpoint
//!   cargo run --release -- --endpoint 8080 # Start with HTTP endpoint on port 8080
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string

use flomon_service::daemon::Daemon;
use flomon_service::endpoint;
use std::env;

fn main() {
    println!("🌊 Flood Monitoring Service");
    println!("============================\n");
    
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
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
                eprintln!("Usage: {} [--endpoint PORT]", args[0]);
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
    
    // Start HTTP endpoint if requested
    if let Some(port) = endpoint_port {
        println!("🚀 Starting HTTP endpoint server...");
        
        // Get a new database connection for the endpoint
        match flomon_service::db::connect_with_validation() {
            Ok(client) => {
                if let Err(e) = endpoint::start_endpoint_server(port, client) {
                    eprintln!("❌ Endpoint server error: {}", e);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to database for endpoint: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("📋 Startup checks:");
        println!("   ⚠️  Staleness check: not yet implemented");
        println!("   ⚠️  Backfill missing data: not yet implemented");
        println!("   ⚠️  Continuous monitoring: not yet implemented\n");
        
        println!("ℹ️  Run tests to guide implementation:");
        println!("   cargo test --test daemon_lifecycle\n");
        println!("ℹ️  Start with HTTP endpoint:");
        println!("   cargo run --release -- --endpoint 8080\n");
    }
}


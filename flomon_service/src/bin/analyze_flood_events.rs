#!/usr/bin/env rust
//! Flood Event Analysis
//!
//! Analyzes historical flood events to identify precursor patterns and
//! correlate multi-source data (USGS + USACE CWMS).
//!
//! For each historical flood event:
//! 1. Detect precursor window (when did significant rise begin?)
//! 2. Collect all USGS observations during event window
//! 3. Collect USACE CWMS data if available (backwater analysis)
//! 4. Compute metrics (rise rate, duration, peak stats)
//! 5. Detect precursor conditions (rapid rise, backwater onset)
//! 6. Populate flood_analysis schema with relational data
//!
//! Usage:
//!   cargo run --bin analyze_flood_events
//!
//! Options:
//!   --site-code SITE    Only analyze events for specific site
//!   --reanalyze         Re-analyze events already in flood_analysis schema
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string

use flomon_service::analysis::flood_events::{
    load_config, load_historical_events, analyze_event,
};
use postgres::Client;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 Flood Event Analysis");
    println!("=======================\n");
    
    // Parse arguments
    let args: Vec<String> = env::args().collect();
    let site_filter = args.iter()
        .position(|a| a == "--site-code")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.clone());
    
    let reanalyze = args.contains(&"--reanalyze".to_string());
    
    // Connect to database with validation
    println!("📊 Connecting to database...");
    let mut client = flomon_service::db::connect_and_verify(&["nws", "usgs_raw", "flood_analysis"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", e);
            eprintln!("\nNote: flood_analysis schema required. Run migration:");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/005_flood_analysis.sql\n");
            eprintln!("Then run permission script:");
            eprintln!("  psql -U postgres -d flopro_db -f scripts/grant_permissions.sql\n");
            std::process::exit(1);
        });
    println!("✓ Connected\n");
    
    // Load analysis configuration
    println!("⚙️  Loading analysis configuration...");
    let config = load_config(&mut client)?;
    println!("✓ Configuration loaded:");
    println!("  - Precursor lookback: {} days", config.precursor_lookback_days);
    println!("  - Rise threshold: {:.2} ft", config.significant_rise_threshold_ft);
    println!("  - Rise rate threshold: {:.2} ft/day", config.rise_rate_threshold_ft_per_day);
    println!("  - Post-peak window: {} days\n", config.post_peak_window_days);
    
    // Clear existing analysis if reanalyzing
    if reanalyze {
        println!("🔄 Re-analysis mode: clearing existing analysis...");
        client.execute("DELETE FROM flood_analysis.events", &[])?;
        println!("✓ Cleared\n");
    }
    
    // Load historical events to analyze
    println!("📋 Loading historical flood events...");
    let mut events = load_historical_events(&mut client)?;
    
    // Filter by site if requested
    if let Some(site_code) = &site_filter {
        events.retain(|e| &e.site_code == site_code);
        println!("✓ Found {} events for site {}\n", events.len(), site_code);
    } else {
        println!("✓ Found {} events to analyze\n", events.len());
    }
    
    if events.is_empty() {
        println!("ℹ️  No events to analyze.");
        if !reanalyze {
            println!("   All events may already be analyzed. Use --reanalyze to re-process.\n");
        }
        return Ok(());
    }
    
    // Analyze each event
    println!("🔍 Analyzing flood events...\n");
    let mut success_count = 0;
    let mut error_count = 0;
    
    for event in &events {
        match analyze_event(&mut client, event, &config) {
            Ok(_) => success_count += 1,
            Err(e) => {
                eprintln!("  ✗ Error analyzing {}: {}", event.site_code, e);
                error_count += 1;
            }
        }
    }
    
    // Correlate CWMS data if available
    println!("\n🔗 Correlating USACE CWMS data...");
    let cwms_correlation_result = correlate_cwms_data(&mut client);
    match cwms_correlation_result {
        Ok(count) => println!("✓ Linked {} CWMS observations to events", count),
        Err(e) => println!("⚠ CWMS correlation skipped: {}", e),
    }
    
    // Compute event metrics
    println!("\n📈 Computing event metrics...");
    let metrics_result = compute_event_metrics(&mut client);
    match metrics_result {
        Ok(count) => println!("✓ Computed metrics for {} events", count),
        Err(e) => eprintln!("✗ Error computing metrics: {}", e),
    }
    
    // Summary
    println!("\n{}", "=".repeat(50));
    println!("Summary:");
    println!("  Successfully analyzed: {}", success_count);
    println!("  Errors: {}", error_count);
    println!("{}", "=".repeat(50));
    
    if success_count > 0 {
        println!("\nQuery examples:");
        println!("  -- View all analyzed events");
        println!("  SELECT * FROM flood_analysis.event_summary;");
        println!();
        println!("  -- Events with backwater influence");
        println!("  SELECT * FROM flood_analysis.backwater_influenced_events;");
        println!();
        println!("  -- Observations for a specific event");
        println!("  SELECT * FROM flood_analysis.event_observations WHERE event_id = 1;");
    }
    
    Ok(())
}

/// Correlate USACE CWMS data to flood events
fn correlate_cwms_data(client: &mut Client) -> Result<i32, Box<dyn std::error::Error>> {
    // Check if usace schema exists
    let schema_exists: bool = client.query_one(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'usace')",
        &[],
    )?.get(0);
    
    if !schema_exists {
        return Err("USACE schema not found".into());
    }
    
    // For each event, find CWMS data in the event window
    let result = client.execute(
        "INSERT INTO flood_analysis.event_cwms_data 
         (event_id, location_id, location_name, river_name, timestamp, 
          parameter_type, value, unit, hours_before_peak)
         SELECT 
             e.event_id,
             t.location_id,
             l.location_name,
             l.river_name,
             t.timestamp,
             t.parameter_type,
             t.value,
             t.unit,
             EXTRACT(EPOCH FROM (e.event_peak - t.timestamp)) / 3600.0 AS hours_before_peak
         FROM flood_analysis.events e
         JOIN usace.cwms_timeseries t ON 
             t.timestamp BETWEEN e.precursor_window_start AND e.event_end
         JOIN usace.cwms_locations l ON t.location_id = l.location_id
         WHERE l.affects_illinois = true
         ON CONFLICT (event_id, location_id, timestamp, parameter_type) DO NOTHING",
        &[],
    )?;
    
    // Update has_backwater_data flag
    client.execute(
        "UPDATE flood_analysis.events e
         SET has_backwater_data = true
         WHERE EXISTS (
             SELECT 1 FROM flood_analysis.event_cwms_data c
             WHERE c.event_id = e.event_id
         )",
        &[],
    )?;
    
    Ok(result as i32)
}

/// Compute aggregate metrics for each event
fn compute_event_metrics(client: &mut Client) -> Result<i32, Box<dyn std::error::Error>> {
    let result = client.execute(
        "INSERT INTO flood_analysis.event_metrics 
         (event_id, initial_stage_ft, peak_stage_ft, total_rise_ft,
          rise_duration_hours, avg_rise_rate_ft_per_day, max_single_day_rise_ft,
          peak_stage_timestamp, observation_count, data_completeness_pct)
         SELECT 
             e.event_id,
             MIN(o.stage_ft) FILTER (WHERE o.phase = 'precursor') as initial_stage_ft,
             e.peak_stage_ft,
             e.total_rise_ft,
             e.rise_duration_hours,
             e.average_rise_rate_ft_per_day,
             e.max_rise_rate_ft_per_day,
             e.event_peak,
             COUNT(o.observation_id) as observation_count,
             CASE 
                 WHEN e.rise_duration_hours > 0 
                 THEN LEAST(100.0, (COUNT(o.observation_id)::numeric / (e.rise_duration_hours / 6.0)) * 100.0)
                 ELSE 0.0
             END as data_completeness_pct
         FROM flood_analysis.events e
         LEFT JOIN flood_analysis.event_observations o ON e.event_id = o.event_id
         WHERE NOT EXISTS (
             SELECT 1 FROM flood_analysis.event_metrics m
             WHERE m.event_id = e.event_id
         )
         GROUP BY e.event_id
         ON CONFLICT (event_id) DO NOTHING",
        &[],
    )?;
    
    Ok(result as i32)
}

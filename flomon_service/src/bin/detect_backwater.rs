#!/usr/bin/env rust
//! Backwater Flood Detection
//!
//! Analyzes Mississippi River and Illinois River stage data to detect
//! "bottom-up" backwater flooding where high Mississippi River levels
//! back water up into the Illinois River.
//!
//! This happens when Mississippi stage exceeds Illinois stage, reversing
//! the normal downstream gradient. Critical for flood prediction near
//! the confluence at Grafton, IL.
//!
//! Usage:
//!   cargo run --bin detect_backwater
//!
//! Environment:
//!   DATABASE_URL - PostgreSQL connection string

use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌊 Mississippi River Backwater Detection");
    println!("=========================================\n");
    
    let mut client = flomon_service::db::connect_and_verify(&["usace"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", e);
            eprintln!("\nRun setup validation: ./scripts/validate_db_setup.sh\n");
            std::process::exit(1);
        });
    
    // Check Mississippi River current conditions
    println!("📊 Current Mississippi River Conditions:");
    println!("─────────────────────────────────────────");
    
    let miss_rows = client.query(
        "SELECT 
            l.location_name,
            l.river_mile,
            t.timestamp,
            t.value as stage_ft,
            t.unit,
            AGE(NOW(), t.timestamp) as data_age
         FROM usace.cwms_locations l
         JOIN LATERAL (
            SELECT timestamp, value, unit
            FROM usace.cwms_timeseries ts
            WHERE ts.location_id = l.location_id
              AND ts.parameter_id = 'Stage'
            ORDER BY timestamp DESC
            LIMIT 1
         ) t ON true
         WHERE l.river_name = 'Mississippi River'
           AND l.monitored = true
         ORDER BY l.river_mile DESC",
        &[],
    )?;
    
    for row in &miss_rows {
        let location: String = row.get(0);
        let river_mile: Option<f64> = row.get(1);
        let timestamp: chrono::DateTime<Utc> = row.get(2);
        let stage: f64 = row.get(3);
        let unit: String = row.get(4);
        
        println!("  {} (Mile {:.1}):", location, river_mile.unwrap_or(0.0));
        println!("    Stage: {:.2} {}", stage, unit);
        println!("    Time:  {}", timestamp.format("%Y-%m-%d %H:%M UTC"));
        println!();
    }
    
    // Check Illinois River near confluence
    println!("\n📊 Illinois River Conditions (Near Confluence):");
    println!("───────────────────────────────────────────────");
    
    let il_rows = client.query(
        "SELECT 
            s.site_name,
            g.value as stage_ft,
            g.reading_time,
            AGE(NOW(), g.reading_time) as data_age
         FROM usgs_raw.sites s
         JOIN LATERAL (
            SELECT value, reading_time
            FROM usgs_raw.gauge_readings gr
            WHERE gr.site_code = s.site_code
              AND gr.parameter_code = '00065'  -- stage
            ORDER BY reading_time DESC
            LIMIT 1
         ) g ON true
         WHERE s.site_code IN ('05586100', '05585500')  -- Grafton area stations
         ORDER BY g.reading_time DESC",
        &[],
    )?;
    
    for row in &il_rows {
        let site_name: String = row.get(0);
        let stage: f64 = row.get(1);
        let timestamp: chrono::DateTime<Utc> = row.get(2);
        
        println!("  {}:", site_name);
        println!("    Stage: {:.2} ft", stage);
        println!("    Time:  {}", timestamp.format("%Y-%m-%d %H:%M UTC"));
        println!();
    }
    
    // Detect backwater conditions
    println!("\n🔍 Backwater Analysis:");
    println!("──────────────────────");
    
    // Get Grafton Mississippi stage
    let grafton_miss = client.query_opt(
        "SELECT value 
         FROM usace.cwms_timeseries 
         WHERE location_id LIKE 'Grafton%' 
           AND parameter_id = 'Stage'
         ORDER BY timestamp DESC 
         LIMIT 1",
        &[],
    )?;
    
    // Get Illinois River stage near confluence
    let il_stage = client.query_opt(
        "SELECT value 
         FROM usgs_raw.gauge_readings 
         WHERE site_code = '05586100'  -- IL River at Grafton
           AND parameter_code = '00065'
         ORDER BY reading_time DESC 
         LIMIT 1",
        &[],
    )?;
    
    if let (Some(miss_row), Some(il_row)) = (grafton_miss, il_stage) {
        let miss_stage: f64 = miss_row.get(0);
        let il_stage: f64 = il_row.get(0);
        let differential = miss_stage - il_stage;
        
        println!("  Mississippi River at Grafton: {:.2} ft", miss_stage);
        println!("  Illinois River at Grafton:    {:.2} ft", il_stage);
        println!("  Differential:                  {:.2} ft", differential);
        println!();
        
        let backwater_detected = differential > 2.0;
        let severity = if differential > 10.0 {
            "EXTREME"
        } else if differential > 5.0 {
            "MAJOR"
        } else if differential > 2.0 {
            "MODERATE"
        } else {
            "MINOR"
        };
        
        if backwater_detected {
            println!("  ⚠️  BACKWATER CONDITIONS DETECTED");
            println!("  Severity: {}", severity);
            println!();
            println!("  💡 Mississippi River is backing water up into Illinois River.");
            println!("     This can increase flood levels along lower Illinois River.");
        } else {
            println!("  ✓ Normal conditions - no backwater detected");
            println!("    (Illinois River flowing normally into Mississippi)");
        }
    } else {
        println!("  ⚠️  Insufficient data for backwater analysis");
    }
    
    // Check for active backwater events
    println!("\n\n📋 Active Backwater Events:");
    println!("──────────────────────────");
    
    let active_events = client.query(
        "SELECT 
            event_start,
            mississippi_location_id,
            mississippi_peak_ft,
            elevation_above_normal_ft,
            backwater_severity,
            gradient_reversal,
            event_name
         FROM usace.backwater_events
         WHERE event_end IS NULL
         ORDER BY event_start DESC",
        &[],
    )?;
    
    if active_events.is_empty() {
        println!("  No active backwater events recorded\n");
    } else {
        for row in active_events {
            let start: chrono::DateTime<Utc> = row.get(0);
            let location: String = row.get(1);
            let peak: f64 = row.get(2);
            let above_normal: Option<f64> = row.get(3);
            let severity: Option<String> = row.get(4);
            let gradient_reversal: bool = row.get(5);
            let name: Option<String> = row.get(6);
            
            println!("  Event: {}", name.unwrap_or_else(|| "Unnamed".to_string()));
            println!("    Started:        {}", start.format("%Y-%m-%d %H:%M UTC"));
            println!("    Location:       {}", location);
            println!("    Peak Stage:     {:.2} ft", peak);
            if let Some(above) = above_normal {
                println!("    Above Normal:  +{:.2} ft", above);
            }
            if let Some(sev) = severity {
                println!("    Severity:       {}", sev.to_uppercase());
            }
            println!("    Flow Reversal:  {}", if gradient_reversal { "YES" } else { "NO" });
            println!();
        }
    }
    
    Ok(())
}

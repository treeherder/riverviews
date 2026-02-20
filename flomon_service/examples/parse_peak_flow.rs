/// Example: Parse USGS Peak Streamflow RDB data and identify flood events
///
/// Usage:
///   cargo run --example parse_peak_flow -- <rdb_file>
///
/// This demonstrates the peak_flow parser module using real USGS RDB data.
/// Shows:
///   - Total peaks parsed
///   - Flood events detected (stage >= flood_stage_ft)
///   - Severity classification (flood, moderate, major)
///   - Notable historic floods

use flomon_service::ingest::peak_flow::{
    parse_rdb, identify_flood_events, FloodThresholds, FloodSeverity,
};
use chrono::Datelike;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <rdb_file>", args[0]);
        eprintln!();
        eprintln!("Example RDB files can be downloaded from:");
        eprintln!("  https://nwis.waterdata.usgs.gov/il/nwis/peak?site_no=05567500&agency_cd=USGS&format=rdb");
        std::process::exit(1);
    }
    
    let filename = &args[1];
    let rdb_text = fs::read_to_string(filename)
        .expect("Failed to read RDB file");
    
    println!("Parsing USGS Peak Streamflow data from: {}\n", filename);
    
    // Parse RDB data
    let records = parse_rdb(&rdb_text)
        .expect("Failed to parse RDB data");
    
    println!("✓ Parsed {} annual peak records", records.len());
    
    if records.is_empty() {
        println!("No data found in file");
        return;
    }
    
    // Get site code from first record
    let site_code = &records[0].site_code;
    let earliest = records.iter().map(|r| r.peak_date.year()).min().unwrap();
    let latest = records.iter().map(|r| r.peak_date.year()).max().unwrap();
    
    println!("Site: {}", site_code);
    println!("Period of record: {}-{} ({} years)\n", earliest, latest, latest - earliest + 1);
    
    // Define thresholds based on site
    // (In real implementation, these would come from stations.toml)
    let thresholds = match site_code.as_str() {
        "05567500" => Some(FloodThresholds {
            flood_stage_ft: 18.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 22.0,
        }),
        "05568500" => Some(FloodThresholds {
            flood_stage_ft: 16.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 24.0,
        }),
        "05568000" => Some(FloodThresholds {
            flood_stage_ft: 15.0,
            moderate_flood_stage_ft: 19.0,
            major_flood_stage_ft: 23.0,
        }),
        "05557000" => Some(FloodThresholds {
            flood_stage_ft: 15.0,
            moderate_flood_stage_ft: 19.0,
            major_flood_stage_ft: 22.0,
        }),
        "05552500" => Some(FloodThresholds {
            flood_stage_ft: 14.0,
            moderate_flood_stage_ft: 18.0,
            major_flood_stage_ft: 22.0,
        }),
        _ => {
            println!("⚠ No flood thresholds defined for site {}", site_code);
            println!("Cannot identify flood events - only monitoring absolute peaks");
            return;
        }
    };
    
    let thresholds = thresholds.unwrap();
    
    // Identify flood events
    let events = identify_flood_events(&records, &thresholds);
    
    println!("FLOOD EVENT DETECTION");
    println!("=====================");
    println!("Thresholds: Flood={:.1}' | Moderate={:.1}' | Major={:.1}'\n",
        thresholds.flood_stage_ft,
        thresholds.moderate_flood_stage_ft,
        thresholds.major_flood_stage_ft
    );
    
    if events.is_empty() {
        println!("✓ No flood events detected (all peaks below {:.1} ft flood stage)",
            thresholds.flood_stage_ft);
        return;
    }
    
    println!("Found {} flood events:\n", events.len());
    
    // Count by severity
    let flood_count = events.iter().filter(|e| e.severity == FloodSeverity::Flood).count();
    let moderate_count = events.iter().filter(|e| e.severity == FloodSeverity::Moderate).count();
    let major_count = events.iter().filter(|e| e.severity == FloodSeverity::Major).count();
    
    println!("Severity breakdown:");
    println!("  Major floods:    {} ({:.1}%)", major_count, major_count as f64 / events.len() as f64 * 100.0);
    println!("  Moderate floods: {} ({:.1}%)", moderate_count, moderate_count as f64 / events.len() as f64 * 100.0);
    println!("  Minor floods:    {} ({:.1}%)", flood_count, flood_count as f64 / events.len() as f64 * 100.0);
    println!();
    
    // Show top 10 worst floods
    let mut sorted_events = events.clone();
    sorted_events.sort_by(|a, b| b.peak_stage_ft.partial_cmp(&a.peak_stage_ft).unwrap());
    
    println!("TOP 10 WORST FLOODS (by peak stage):");
    println!("{:<12} {:<10} {:>10}", "Date", "Severity", "Stage (ft)");
    println!("{}", "-".repeat(35));
    
    for event in sorted_events.iter().take(10) {
        let severity_str = match event.severity {
            FloodSeverity::Major => "MAJOR",
            FloodSeverity::Moderate => "Moderate",
            FloodSeverity::Flood => "Flood",
        };
        
        println!("{:<12} {:<10} {:>10.2}",
            event.crest_time.format("%Y-%m-%d"),
            severity_str,
            event.peak_stage_ft
        );
    }
    
    println!();
    
    // Recent floods (last 20 years)
    let recent_year = latest - 20;
    let recent_events: Vec<_> = events.iter()
        .filter(|e| e.crest_time.date().year() >= recent_year)
        .collect();
    
    if !recent_events.is_empty() {
        println!("RECENT FLOODS (last 20 years):");
        println!("{:<12} {:<10} {:>10}", "Date", "Severity", "Stage (ft)");
        println!("{}", "-".repeat(35));
        
        for event in recent_events.iter() {
            let severity_str = match event.severity {
                FloodSeverity::Major => "MAJOR",
                FloodSeverity::Moderate => "Moderate",
                FloodSeverity::Flood => "Flood",
            };
            
            println!("{:<12} {:<10} {:>10.2}",
                event.crest_time.format("%Y-%m-%d"),
                severity_str,
                event.peak_stage_ft
            );
        }
    }
}

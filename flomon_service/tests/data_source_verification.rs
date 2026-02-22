//! Data Source Verification Integration Tests
//!
//! These tests verify which configured data sources are actually accessible
//! and returning data. Run these before adding new data sources to validate
//! the architecture.

use flomon_service::verify::*;

#[test]
fn test_usgs_verification() {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let stations = flomon_service::stations::load_stations();
    
    println!("\n🔍 Testing USGS Stations:");
    println!("═══════════════════════════════════════════════════════════");
    
    let mut working = 0;
    let mut failed = 0;
    
    for station in &stations {
        let result = verify_usgs_station(
            &client,
            &station.site_code,
            &station.name,
            &station.expected_parameters,
        );
        
        println!("\n{} ({})", station.name, station.site_code);
        println!("  Status: {:?}", result.status);
        println!("  Site Exists: {}", result.site_exists);
        println!("  Parameters: {} available, {} missing",
            result.parameters_available.len(),
            result.parameters_missing.len());
        println!("  Sample Data: {} readings", result.sample_data_count);
        println!("  Peak Flow: {}", if result.peak_flow_available { "Available" } else { "Not available" });
        
        if let Some(error) = &result.error_message {
            println!("  Error: {}", error);
        }
        
        match result.status {
            VerificationStatus::Success | VerificationStatus::PartialSuccess => working += 1,
            VerificationStatus::Failed => failed += 1,
        }
    }
    
    println!("\n═══════════════════════════════════════════════════════════");
    println!("Summary: {}/{} working, {} failed", working, stations.len(), failed);
    println!("═══════════════════════════════════════════════════════════\n");
    
    // At least some stations should be working
    assert!(working > 0, "No USGS stations are working!");
}

#[test]
fn test_cwms_verification() {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let locations = flomon_service::usace_locations::load_locations().unwrap();
    
    println!("\n🔍 Testing CWMS Locations:");
    println!("═══════════════════════════════════════════════════════════");
    
    let mut working = 0;
    let mut failed = 0;
    
    for location in &locations {
        let result = verify_cwms_location(
            &client,
            &location.name,
            &location.office,
            &location.cwms_location,
        );
        
        println!("\n{}", location.name);
        println!("  Office: {}, Location: {}", location.office, location.cwms_location);
        println!("  Status: {:?}", result.status);
        println!("  Catalog Found: {}", result.catalog_found);
        println!("  Timeseries: {}", result.timeseries_discovered.len());
        
        if !result.timeseries_discovered.is_empty() {
            println!("    Discovered:");
            for ts in &result.timeseries_discovered {
                println!("      - {}", ts);
            }
        }
        
        println!("  Sample Data: {} points", result.sample_data_count);
        
        if let Some(error) = &result.error_message {
            println!("  Error: {}", error);
        }
        
        match result.status {
            VerificationStatus::Success | VerificationStatus::PartialSuccess => working += 1,
            VerificationStatus::Failed => failed += 1,
        }
    }
    
    println!("\n═══════════════════════════════════════════════════════════");
    println!("Summary: {}/{} working, {} failed", working, locations.len(), failed);
    println!("═══════════════════════════════════════════════════════════\n");
    
    // This test documents what works - it doesn't fail if CWMS is unavailable
    println!("Note: CWMS verification complete. Check output above for availability.");
}

#[test]
fn test_asos_verification() {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let stations = flomon_service::asos_locations::load_locations("./iem_asos.toml").unwrap();
    
    println!("\n🔍 Testing ASOS Stations:");
    println!("═══════════════════════════════════════════════════════════");
    
    let mut working = 0;
    let mut failed = 0;
    
    for station in &stations {
        let result = verify_asos_station(
            &client,
            &station.station_id,
            &station.name,
        );
        
        println!("\n{} ({})", station.name, station.station_id);
        println!("  Status: {:?}", result.status);
        println!("  API Responsive: {}", result.api_responsive);
        println!("  Sample Data: {} observations", result.sample_data_count);
        println!("  Data Types: {:?}", result.data_types_available);
        
        if let Some(error) = &result.error_message {
            println!("  Error: {}", error);
        }
        
        match result.status {
            VerificationStatus::Success | VerificationStatus::PartialSuccess => working += 1,
            VerificationStatus::Failed => failed += 1,
        }
    }
    
    println!("\n═══════════════════════════════════════════════════════════");
    println!("Summary: {}/{} working, {} failed", working, stations.len(), failed);
    println!("═══════════════════════════════════════════════════════════\n");
    
    // At least some ASOS stations should be working
    assert!(working > 0, "No ASOS stations are working!");
}

#[test]
fn test_full_verification_report() {
    println!("\n🚀 Running Full Data Source Verification");
    println!("═══════════════════════════════════════════════════════════\n");
    
    let report = run_full_verification().expect("Verification failed");
    
    print_summary(&report);
    
    // Save report to file
    let report_json = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write("verification_report.json", report_json).unwrap();
    
    println!("\n📄 Full report saved to: verification_report.json\n");
    
    // At least some data sources should be working
    let total_working = report.summary.usgs_working + 
                       report.summary.cwms_working + 
                       report.summary.asos_working;
    
    assert!(total_working > 0, "No data sources are working!");
}

#[test]
fn test_generate_markdown_report() {
    let report = run_full_verification().expect("Verification failed");
    
    let mut md = String::new();
    md.push_str("# Data Source Verification Report\n\n");
    md.push_str(&format!("**Generated:** {}\n\n", report.timestamp));
    
    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **USGS Stations:** {}/{} working ({} failed)\n",
        report.summary.usgs_working, report.summary.usgs_total, report.summary.usgs_failed));
    md.push_str(&format!("- **CWMS Locations:** {}/{} working ({} failed)\n",
        report.summary.cwms_working, report.summary.cwms_total, report.summary.cwms_failed));
    md.push_str(&format!("- **ASOS Stations:** {}/{} working ({} failed)\n\n",
        report.summary.asos_working, report.summary.asos_total, report.summary.asos_failed));
    
    md.push_str("## USGS Stations\n\n");
    md.push_str("| Site Code | Name | Status | Data | Parameters |\n");
    md.push_str("|-----------|------|--------|------|------------|\n");
    
    for result in &report.usgs_results {
        let status_icon = match result.status {
            VerificationStatus::Success => "✅",
            VerificationStatus::PartialSuccess => "⚠️",
            VerificationStatus::Failed => "❌",
        };
        
        md.push_str(&format!("| {} | {} | {} | {} readings | {}/{} |\n",
            result.site_code,
            result.name,
            status_icon,
            result.sample_data_count,
            result.parameters_available.len(),
            result.parameters_expected.len()));
    }
    
    md.push_str("\n## CWMS Locations\n\n");
    md.push_str("| Name | Office | Status | Timeseries | Data |\n");
    md.push_str("|------|--------|--------|------------|------|\n");
    
    for result in &report.cwms_results {
        let status_icon = match result.status {
            VerificationStatus::Success => "✅",
            VerificationStatus::PartialSuccess => "⚠️",
            VerificationStatus::Failed => "❌",
        };
        
        md.push_str(&format!("| {} | {} | {} | {} | {} points |\n",
            result.name,
            result.office,
            status_icon,
            result.timeseries_discovered.len(),
            result.sample_data_count));
    }
    
    md.push_str("\n## ASOS Stations\n\n");
    md.push_str("| Station | Name | Status | Observations | Data Types |\n");
    md.push_str("|---------|------|--------|--------------|------------|\n");
    
    for result in &report.asos_results {
        let status_icon = match result.status {
            VerificationStatus::Success => "✅",
            VerificationStatus::PartialSuccess => "⚠️",
            VerificationStatus::Failed => "❌",
        };
        
        md.push_str(&format!("| {} | {} | {} | {} | {} |\n",
            result.station_id,
            result.name,
            status_icon,
            result.sample_data_count,
            result.data_types_available.join(", ")));
    }
    
    std::fs::write("VERIFICATION_REPORT.md", md).unwrap();
    println!("\n📄 Markdown report saved to: VERIFICATION_REPORT.md\n");
}

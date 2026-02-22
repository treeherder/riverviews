//! Data Source Verification Module
//!
//! Framework for testing configuration files against live APIs to determine
//! which configured stations/locations are accessible and returning data.
//!
//! Use this before adding new data sources to validate the architecture.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

// ============================================================================
// Verification Results
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub timestamp: String,
    pub usgs_results: Vec<UsgsVerification>,
    pub cwms_results: Vec<CwmsVerification>,
    pub asos_results: Vec<AsosVerification>,
    pub summary: VerificationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub usgs_total: usize,
    pub usgs_working: usize,
    pub usgs_failed: usize,
    pub cwms_total: usize,
    pub cwms_working: usize,
    pub cwms_failed: usize,
    pub asos_total: usize,
    pub asos_working: usize,
    pub asos_failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsgsVerification {
    pub site_code: String,
    pub name: String,
    pub status: VerificationStatus,
    pub site_exists: bool,
    pub parameters_available: Vec<String>,
    pub parameters_expected: Vec<String>,
    pub parameters_missing: Vec<String>,
    pub sample_data_count: usize,
    pub peak_flow_available: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CwmsVerification {
    pub name: String,
    pub office: String,
    pub cwms_location: String,
    pub status: VerificationStatus,
    pub catalog_found: bool,
    pub timeseries_discovered: Vec<String>,
    pub sample_data_available: bool,
    pub sample_data_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsosVerification {
    pub station_id: String,
    pub name: String,
    pub status: VerificationStatus,
    pub api_responsive: bool,
    pub sample_data_count: usize,
    pub data_types_available: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationStatus {
    Success,
    PartialSuccess,
    Failed,
}

// ============================================================================
// USGS Verification
// ============================================================================

pub fn verify_usgs_station(
    client: &reqwest::blocking::Client,
    site_code: &str,
    name: &str,
    expected_parameters: &[String],
) -> UsgsVerification {
    let mut result = UsgsVerification {
        site_code: site_code.to_string(),
        name: name.to_string(),
        status: VerificationStatus::Failed,
        site_exists: false,
        parameters_available: Vec::new(),
        parameters_expected: expected_parameters.to_vec(),
        parameters_missing: Vec::new(),
        sample_data_count: 0,
        peak_flow_available: false,
        error_message: None,
    };

    // Test 1: Check if site exists with instantaneous values
    let param_strs: Vec<&str> = expected_parameters.iter().map(|s| s.as_str()).collect();
    let iv_url = crate::ingest::usgs::build_iv_url(
        &[site_code],
        &param_strs,
        "PT4H",
    );

    match client.get(&iv_url).timeout(Duration::from_secs(10)).send() {
        Ok(response) => {
            if response.status().is_success() {
                result.site_exists = true;
                
                // Try to parse response as generic JSON to extract what we need
                match response.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(time_series) = json
                            .get("value")
                            .and_then(|v| v.get("timeSeries"))
                            .and_then(|ts| ts.as_array())
                        {
                            for ts in time_series {
                                // Extract parameter code
                                if let Some(param_code) = ts
                                    .get("variable")
                                    .and_then(|v| v.get("variableCode"))
                                    .and_then(|vc| vc.as_array())
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|code| code.get("value"))
                                    .and_then(|val| val.as_str())
                                {
                                    result.parameters_available.push(param_code.to_string());
                                }
                                
                                // Count data points
                                if let Some(values_arr) = ts
                                    .get("values")
                                    .and_then(|v| v.as_array())
                                {
                                    for val_set in values_arr {
                                        if let Some(values) = val_set
                                            .get("value")
                                            .and_then(|v| v.as_array())
                                        {
                                            result.sample_data_count += values.len();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        result.error_message = Some(format!("Parse error: {}", e));
                    }
                }
            } else {
                result.error_message = Some(format!("HTTP {}", response.status()));
            }
        }
        Err(e) => {
            result.error_message = Some(format!("Request failed: {}", e));
        }
    }

    // Test 2: Check peak flow availability
    let peak_url = format!(
        "https://nwis.waterdata.usgs.gov/nwis/peak?site_no={}&agency_cd=USGS&format=rdb",
        site_code
    );
    
    if let Ok(response) = client.get(&peak_url).timeout(Duration::from_secs(10)).send() {
        if response.status().is_success() {
            result.peak_flow_available = true;
        }
    }

    // Determine missing parameters
    for expected in expected_parameters {
        if !result.parameters_available.contains(expected) {
            result.parameters_missing.push(expected.clone());
        }
    }

    // Determine status
    if result.site_exists && result.sample_data_count > 0 {
        if result.parameters_missing.is_empty() {
            result.status = VerificationStatus::Success;
        } else {
            result.status = VerificationStatus::PartialSuccess;
        }
    }

    result
}

// ============================================================================
// CWMS Verification
// ============================================================================

pub fn verify_cwms_location(
    client: &reqwest::blocking::Client,
    name: &str,
    office: &str,
    cwms_location: &str,
) -> CwmsVerification {
    let mut result = CwmsVerification {
        name: name.to_string(),
        office: office.to_string(),
        cwms_location: cwms_location.to_string(),
        status: VerificationStatus::Failed,
        catalog_found: false,
        timeseries_discovered: Vec::new(),
        sample_data_available: false,
        sample_data_count: 0,
        error_message: None,
    };

    // Test 1: Query catalog for this location
    let pattern = format!("{}.*", cwms_location);
    match crate::ingest::cwms::discover_timeseries(client, office, &pattern) {
        Ok(timeseries) => {
            if !timeseries.is_empty() {
                result.catalog_found = true;
                result.timeseries_discovered = timeseries.clone();

                // Test 2: Try to fetch sample data from first discovered timeseries
                if let Some(first_ts) = timeseries.first() {
                    let end = Utc::now();
                    let begin = end - chrono::Duration::hours(24);
                    
                    let data_url = format!(
                        "https://cwms-data.usace.army.mil/cwms-data/timeseries?name={}&office={}&begin={}&end={}",
                        first_ts,
                        office,
                        begin.format("%Y-%m-%dT%H:%M:%S"),
                        end.format("%Y-%m-%dT%H:%M:%S")
                    );

                    if let Ok(response) = client.get(&data_url).timeout(Duration::from_secs(10)).send() {
                        if response.status().is_success() {
                            if let Ok(ts_data) = response.json::<crate::ingest::cwms::CwmsTimeseriesResponse>() {
                                if let Some(values) = ts_data.values {
                                    result.sample_data_count = values.len();
                                    if result.sample_data_count > 0 {
                                        result.sample_data_available = true;
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                result.error_message = Some("No timeseries found in catalog".to_string());
            }
        }
        Err(e) => {
            result.error_message = Some(format!("Catalog query failed: {}", e));
        }
    }

    // Determine status
    if result.catalog_found {
        if result.sample_data_available {
            result.status = VerificationStatus::Success;
        } else {
            result.status = VerificationStatus::PartialSuccess;
        }
    }

    result
}

// ============================================================================
// ASOS Verification
// ============================================================================

pub fn verify_asos_station(
    client: &reqwest::blocking::Client,
    station_id: &str,
    name: &str,
) -> AsosVerification {
    let mut result = AsosVerification {
        station_id: station_id.to_string(),
        name: name.to_string(),
        status: VerificationStatus::Failed,
        api_responsive: false,
        sample_data_count: 0,
        data_types_available: Vec::new(),
        error_message: None,
    };

    // Test: Fetch last 4 hours of data
    match crate::ingest::iem::fetch_recent_precip(client, station_id, 4) {
        Ok(observations) => {
            result.api_responsive = true;
            result.sample_data_count = observations.len();

            // Check which data types are populated
            let mut has_temp = false;
            let mut has_precip = false;
            let mut has_wind = false;
            let mut has_pressure = false;

            for obs in &observations {
                if obs.temp_f.is_some() { has_temp = true; }
                if obs.precip_1hr_in.is_some() { has_precip = true; }
                if obs.wind_speed_knots.is_some() { has_wind = true; }
                if obs.pressure_mb.is_some() { has_pressure = true; }
            }

            if has_temp { result.data_types_available.push("temperature".to_string()); }
            if has_precip { result.data_types_available.push("precipitation".to_string()); }
            if has_wind { result.data_types_available.push("wind".to_string()); }
            if has_pressure { result.data_types_available.push("pressure".to_string()); }

            if result.sample_data_count > 0 {
                result.status = VerificationStatus::Success;
            } else {
                result.status = VerificationStatus::PartialSuccess;
            }
        }
        Err(e) => {
            result.error_message = Some(format!("API request failed: {}", e));
        }
    }

    result
}

// ============================================================================
// Full Verification Runner
// ============================================================================

pub fn run_full_verification() -> Result<VerificationReport, Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let mut report = VerificationReport {
        timestamp: Utc::now().to_rfc3339(),
        usgs_results: Vec::new(),
        cwms_results: Vec::new(),
        asos_results: Vec::new(),
        summary: VerificationSummary {
            usgs_total: 0,
            usgs_working: 0,
            usgs_failed: 0,
            cwms_total: 0,
            cwms_working: 0,
            cwms_failed: 0,
            asos_total: 0,
            asos_working: 0,
            asos_failed: 0,
        },
    };

    // Load and verify USGS stations
    println!("🔍 Verifying USGS stations...");
    let usgs_stations = crate::stations::load_stations();
    report.summary.usgs_total = usgs_stations.len();
    
    for station in usgs_stations {
        print!("  {} ... ", station.site_code);
        let result = verify_usgs_station(
            &client,
            &station.site_code,
            &station.name,
            &station.expected_parameters,
        );
        
        match result.status {
            VerificationStatus::Success => {
                println!("✓ OK ({} readings)", result.sample_data_count);
                report.summary.usgs_working += 1;
            }
            VerificationStatus::PartialSuccess => {
                println!("⚠ Partial (missing: {:?})", result.parameters_missing);
                report.summary.usgs_working += 1;
            }
            VerificationStatus::Failed => {
                println!("✗ FAILED: {}", result.error_message.as_deref().unwrap_or("Unknown"));
                report.summary.usgs_failed += 1;
            }
        }
        
        report.usgs_results.push(result);
    }

    // Load and verify CWMS locations
    println!("\n🔍 Verifying CWMS locations...");
    match crate::usace_locations::load_locations() {
        Ok(cwms_locations) => {
            report.summary.cwms_total = cwms_locations.len();
            
            for location in cwms_locations {
                print!("  {} ... ", location.name);
                let result = verify_cwms_location(
                    &client,
                    &location.name,
                    &location.office,
                    &location.cwms_location,
                );
                
                match result.status {
                    VerificationStatus::Success => {
                        println!("✓ OK ({} timeseries, {} data points)", 
                            result.timeseries_discovered.len(), result.sample_data_count);
                        report.summary.cwms_working += 1;
                    }
                    VerificationStatus::PartialSuccess => {
                        println!("⚠ Catalog found but no data ({} timeseries)", 
                            result.timeseries_discovered.len());
                        report.summary.cwms_working += 1;
                    }
                    VerificationStatus::Failed => {
                        println!("✗ FAILED: {}", result.error_message.as_deref().unwrap_or("Unknown"));
                        report.summary.cwms_failed += 1;
                    }
                }
                
                report.cwms_results.push(result);
            }
        }
        Err(e) => {
            println!("⚠ Warning: Could not load CWMS configuration: {}", e);
        }
    }

    // Load and verify ASOS stations
    println!("\n🔍 Verifying ASOS stations...");
    match crate::asos_locations::load_locations("./iem_asos.toml") {
        Ok(asos_stations) => {
            report.summary.asos_total = asos_stations.len();
            
            for station in asos_stations {
                print!("  {} ... ", station.station_id);
                let result = verify_asos_station(
                    &client,
                    &station.station_id,
                    &station.name,
                );
                
                match result.status {
                    VerificationStatus::Success => {
                        println!("✓ OK ({} observations)", result.sample_data_count);
                        report.summary.asos_working += 1;
                    }
                    VerificationStatus::PartialSuccess => {
                        println!("⚠ Responsive but no data");
                        report.summary.asos_working += 1;
                    }
                    VerificationStatus::Failed => {
                        println!("✗ FAILED: {}", result.error_message.as_deref().unwrap_or("Unknown"));
                        report.summary.asos_failed += 1;
                    }
                }
                
                report.asos_results.push(result);
            }
        }
        Err(e) => {
            println!("⚠ Warning: Could not load ASOS configuration: {}", e);
        }
    }

    Ok(report)
}

pub fn print_summary(report: &VerificationReport) {
    println!("\n═══════════════════════════════════════════════════════════");
    println!("📊 VERIFICATION SUMMARY");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("USGS Stations:    {}/{} working  ({} failed)", 
        report.summary.usgs_working, report.summary.usgs_total, report.summary.usgs_failed);
    println!("CWMS Locations:   {}/{} working  ({} failed)", 
        report.summary.cwms_working, report.summary.cwms_total, report.summary.cwms_failed);
    println!("ASOS Stations:    {}/{} working  ({} failed)", 
        report.summary.asos_working, report.summary.asos_total, report.summary.asos_failed);
    println!();
    
    let total_working = report.summary.usgs_working + report.summary.cwms_working + report.summary.asos_working;
    let total_stations = report.summary.usgs_total + report.summary.cwms_total + report.summary.asos_total;
    let success_rate = if total_stations > 0 {
        (total_working as f64 / total_stations as f64) * 100.0
    } else {
        0.0
    };
    
    println!("Overall Success Rate: {:.1}% ({}/{})", success_rate, total_working, total_stations);
    println!("═══════════════════════════════════════════════════════════");
}

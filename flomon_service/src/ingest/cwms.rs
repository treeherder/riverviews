/// USACE CWMS Data API Client
///
/// Retrieves timeseries data from Corps Water Management System (CWMS) API
/// for Mississippi River backwater detection and lock/dam operations monitoring.
///
/// API Documentation: https://cwms-data.usace.army.mil/cwms-data/swagger-ui.html
/// Base URL: https://cwms-data.usace.army.mil/cwms-data/

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

const CWMS_API_BASE: &str = "https://cwms-data.usace.army.mil/cwms-data";

// ============================================================================
// CWMS API Request/Response Structures
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CwmsTimeseriesResponse {
    pub name: String,
    pub office: String,
    pub units: String,
    pub values: Option<Vec<CwmsValue>>,
    #[serde(rename = "value-count")]
    pub value_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CwmsValue {
    #[serde(rename = "date-time")]
    pub date_time: i64,  // Unix timestamp in milliseconds
    pub value: f64,
    pub quality: i32,
}

#[derive(Debug, Clone)]
pub struct CwmsTimeseries {
    pub timeseries_id: String,
    pub location_id: String,
    pub parameter_id: String,    // e.g., "Stage", "Flow", "Elev"
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub unit: String,
    pub quality_code: i32,
}

// ============================================================================
// API Client
// ============================================================================

/// Fetch CWMS timeseries data for a given time range
///
/// # Parameters
/// - `timeseries_id`: Full CWMS timeseries ID (e.g., "Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev")
/// - `office_id`: CWMS office ID (e.g., "MVS")
/// - `begin`: Start time (inclusive)
/// - `end`: End time (inclusive)
pub fn fetch_timeseries(
    client: &reqwest::blocking::Client,
    timeseries_id: &str,
    office_id: &str,
    begin: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<CwmsTimeseries>, Box<dyn std::error::Error>> {
    
    let url = format!(
        "{}/timeseries?name={}&office={}&begin={}&end={}",
        CWMS_API_BASE,
        urlencoding::encode(timeseries_id),
        office_id,
        begin.format("%Y-%m-%dT%H:%M:%S"),
        end.format("%Y-%m-%dT%H:%M:%S")
    );
    
    println!("   Fetching: {}", url);
    
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()?;
    
    if !response.status().is_success() {
        return Err(format!("CWMS API error: {}", response.status()).into());
    }
    
    let api_response: CwmsTimeseriesResponse = response.json()?;
    
    // Parse timeseries ID to extract components
    let parts: Vec<&str> = timeseries_id.split('.').collect();
    let location_id = parts.get(0).unwrap_or(&"unknown").to_string();
    let parameter_id = parts.get(1).unwrap_or(&"unknown").to_string();
    
    let mut records = Vec::new();
    
    if let Some(values) = api_response.values {
        for val in values {
            // Convert milliseconds to DateTime
            let timestamp = DateTime::from_timestamp(val.date_time / 1000, 0)
                .ok_or("Invalid timestamp")?;
            
            records.push(CwmsTimeseries {
                timeseries_id: timeseries_id.to_string(),
                location_id: location_id.clone(),
                parameter_id: parameter_id.clone(),
                timestamp,
                value: val.value,
                unit: api_response.units.clone(),
                quality_code: val.quality,
            });
        }
    }
    
    Ok(records)
}

/// Fetch recent data (last N hours) for a timeseries
pub fn fetch_recent(
    client: &reqwest::blocking::Client,
    timeseries_id: &str,
    office_id: &str,
    hours: i64,
) -> Result<Vec<CwmsTimeseries>, Box<dyn std::error::Error>> {
    
    let end = Utc::now();
    let begin = end - chrono::Duration::hours(hours);
    
    fetch_timeseries(client, timeseries_id, office_id, begin, end)
}

/// Fetch historical data for backfill (date range)
pub fn fetch_historical(
    client: &reqwest::blocking::Client,
    timeseries_id: &str,
    office_id: &str,
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
) -> Result<Vec<CwmsTimeseries>, Box<dyn std::error::Error>> {
    
    let begin = DateTime::<Utc>::from_naive_utc_and_offset(start_date, Utc);
    let end = DateTime::<Utc>::from_naive_utc_and_offset(end_date, Utc);
    
    fetch_timeseries(client, timeseries_id, office_id, begin, end)
}

// ============================================================================
// Backwater Detection Logic
// ============================================================================

/// Compare Mississippi River stage to Illinois River stage
/// Returns true if backwater conditions detected (Mississippi higher than Illinois)
pub fn detect_backwater(
    mississippi_stage_ft: f64,
    illinois_stage_ft: f64,
    threshold_ft: f64,
) -> bool {
    (mississippi_stage_ft - illinois_stage_ft) > threshold_ft
}

/// Classify backwater severity based on stage differential
pub fn classify_backwater_severity(differential_ft: f64) -> &'static str {
    if differential_ft > 10.0 {
        "extreme"
    } else if differential_ft > 5.0 {
        "major"
    } else if differential_ft > 2.0 {
        "moderate"
    } else if differential_ft > 0.5 {
        "minor"
    } else {
        "none"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_backwater() {
        // Mississippi higher than Illinois - backwater
        assert!(detect_backwater(435.0, 430.0, 2.0));
        
        // Illinois higher than Mississippi - normal flow
        assert!(!detect_backwater(430.0, 435.0, 2.0));
        
        // Small differential - no backwater
        assert!(!detect_backwater(431.0, 430.0, 2.0));
    }
    
    #[test]
    fn test_classify_backwater_severity() {
        assert_eq!(classify_backwater_severity(0.3), "none");
        assert_eq!(classify_backwater_severity(1.0), "minor");
        assert_eq!(classify_backwater_severity(3.0), "moderate");
        assert_eq!(classify_backwater_severity(7.0), "major");
        assert_eq!(classify_backwater_severity(12.0), "extreme");
    }
}

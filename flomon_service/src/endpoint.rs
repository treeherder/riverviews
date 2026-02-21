/// HTTP endpoint for querying site data
///
/// Provides a simple REST API for external tools (including Python analysis)
/// to query the current state of monitoring stations.
///
/// Endpoints:
/// - GET /site/{site_code} - Returns all relational data for a site
/// - GET /health - Service health check

use crate::analysis::groupings;
use crate::model::{GaugeReading};
use crate::stations::find_station;
use chrono::{DateTime, Utc};
use postgres::Client;
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

/// Complete site data response including readings, thresholds, and metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct SiteDataResponse {
    /// Site metadata
    pub site_code: String,
    pub site_name: String,
    pub description: String,
    pub latitude: f64,
    pub longitude: f64,
    
    /// Current readings
    pub discharge: Option<ReadingData>,
    pub stage: Option<ReadingData>,
    
    /// Flood thresholds (if defined)
    pub thresholds: Option<ThresholdData>,
    
    /// Monitoring state
    pub monitoring_state: Option<MonitoringStateData>,
    
    /// Data freshness
    pub last_updated: Option<DateTime<Utc>>,
    pub staleness_minutes: Option<i64>,
}

/// Simplified reading data for JSON response
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadingData {
    pub value: f64,
    pub unit: String,
    pub datetime: String,
    pub qualifier: String,
}

/// Threshold data for JSON response
#[derive(Debug, Serialize, Deserialize)]
pub struct ThresholdData {
    pub action_stage_ft: f64,
    pub flood_stage_ft: f64,
    pub moderate_flood_stage_ft: f64,
    pub major_flood_stage_ft: f64,
}

/// Monitoring state for JSON response
#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringStateData {
    pub status: String,
    pub last_poll_attempted: Option<DateTime<Utc>>,
    pub last_poll_succeeded: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
    pub is_stale: bool,
}

// ---------------------------------------------------------------------------
// Data Fetching
// ---------------------------------------------------------------------------

/// Fetch all relational data for a site from the database
pub fn fetch_site_data(client: &mut Client, site_code: &str) -> Result<SiteDataResponse, String> {
    // Get station metadata from registry
    let station = find_station(site_code)
        .ok_or_else(|| format!("Site code {} not found in station registry", site_code))?;
    
    // Fetch latest readings from database
    let readings = fetch_latest_readings(client, site_code)?;
    
    // Group readings by parameter
    let grouped = groupings::group_by_site(readings);
    let site_readings = grouped.get(site_code);
    
    // Extract discharge and stage
    let discharge = site_readings
        .and_then(|sr| sr.discharge_cfs.as_ref())
        .map(reading_to_data);
    
    let stage = site_readings
        .and_then(|sr| sr.stage_ft.as_ref())
        .map(reading_to_data);
    
    // Get monitoring state
    let monitoring_state = fetch_monitoring_state(client, site_code)?;
    
    // Calculate staleness
    let last_updated = site_readings
        .and_then(|sr| {
            sr.stage_ft.as_ref()
                .or(sr.discharge_cfs.as_ref())
                .and_then(|r| chrono::DateTime::parse_from_rfc3339(&r.datetime).ok())
                .map(|dt| dt.with_timezone(&Utc))
        });
    
    let staleness_minutes = last_updated.map(|dt| (Utc::now() - dt).num_minutes());
    
    // Convert thresholds
    let thresholds = station.thresholds.as_ref().map(|t| ThresholdData {
        action_stage_ft: t.action_stage_ft,
        flood_stage_ft: t.flood_stage_ft,
        moderate_flood_stage_ft: t.moderate_flood_stage_ft,
        major_flood_stage_ft: t.major_flood_stage_ft,
    });
    
    Ok(SiteDataResponse {
        site_code: station.site_code.clone(),
        site_name: station.name.clone(),
        description: station.description.clone(),
        latitude: station.latitude,
        longitude: station.longitude,
        discharge,
        stage,
        thresholds,
        monitoring_state,
        last_updated,
        staleness_minutes,
    })
}

/// Fetch latest readings for a site from the database
fn fetch_latest_readings(client: &mut Client, site_code: &str) -> Result<Vec<GaugeReading>, String> {
    let rows = client.query(
        "SELECT DISTINCT ON (parameter_code)
            site_code,
            parameter_code,
            unit,
            value,
            reading_time,
            qualifier
         FROM usgs_raw.gauge_readings
         WHERE site_code = $1
         ORDER BY parameter_code, reading_time DESC",
        &[&site_code]
    ).map_err(|e| format!("Database query failed: {}", e))?;
    
    let mut readings = Vec::new();
    
    for row in rows {
        let site_code: String = row.get(0);
        let parameter_code: String = row.get(1);
        let unit: String = row.get(2);
        let value: rust_decimal::Decimal = row.get(3);
        let reading_time: DateTime<Utc> = row.get(4);
        let qualifier: String = row.get(5);
        
        // Find site name from registry
        let site_name = find_station(&site_code)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| site_code.clone());
        
        readings.push(GaugeReading {
            site_code,
            site_name,
            parameter_code,
            unit,
            value: value.to_string().parse().unwrap_or(0.0),
            datetime: reading_time.to_rfc3339(),
            qualifier,
        });
    }
    
    Ok(readings)
}

/// Fetch monitoring state for a site
fn fetch_monitoring_state(client: &mut Client, site_code: &str) -> Result<Option<MonitoringStateData>, String> {
    let rows = client.query(
        "SELECT status, last_poll_attempted, last_poll_succeeded, consecutive_failures, is_stale
         FROM usgs_raw.monitoring_state
         WHERE site_code = $1",
        &[&site_code]
    ).map_err(|e| format!("Failed to fetch monitoring state: {}", e))?;
    
    if rows.is_empty() {
        return Ok(None);
    }
    
    let row = &rows[0];
    Ok(Some(MonitoringStateData {
        status: row.get(0),
        last_poll_attempted: row.get(1),
        last_poll_succeeded: row.get(2),
        consecutive_failures: row.get(3),
        is_stale: row.get(4),
    }))
}

/// Convert GaugeReading to ReadingData
fn reading_to_data(reading: &GaugeReading) -> ReadingData {
    ReadingData {
        value: reading.value,
        unit: reading.unit.clone(),
        datetime: reading.datetime.clone(),
        qualifier: reading.qualifier.clone(),
    }
}

// ---------------------------------------------------------------------------
// HTTP Server
// ---------------------------------------------------------------------------

/// Start HTTP endpoint server on the specified port
pub fn start_endpoint_server(port: u16, mut client: Client) -> Result<(), String> {
    let server = tiny_http::Server::http(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start HTTP server: {}", e))?;
    
    println!("📡 HTTP endpoint listening on http://0.0.0.0:{}", port);
    println!("   GET /site/{{site_code}} - Query site data");
    println!("   GET /health - Service health check\n");
    
    for request in server.incoming_requests() {
        let url = request.url();
        
        // Route requests
        let response = if url == "/health" {
            handle_health()
        } else if url.starts_with("/site/") {
            let site_code = url.trim_start_matches("/site/");
            handle_site_query(&mut client, site_code)
        } else {
            create_response(
                404,
                serde_json::json!({
                    "error": "Not found",
                    "available_endpoints": ["/health", "/site/{site_code}"]
                })
            )
        };
        
        if let Err(e) = request.respond(response) {
            eprintln!("Failed to send response: {}", e);
        }
    }
    
    Ok(())
}

/// Handle /health endpoint
fn handle_health() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    create_response(
        200,
        serde_json::json!({
            "status": "ok",
            "service": "flomon_service",
            "version": "0.1.0"
        })
    )
}

/// Handle /site/{site_code} endpoint
fn handle_site_query(client: &mut Client, site_code: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match fetch_site_data(client, site_code) {
        Ok(data) => {
            create_response(200, serde_json::to_value(&data).unwrap())
        }
        Err(e) => {
            create_response(
                404,
                serde_json::json!({
                    "error": e,
                    "site_code": site_code
                })
            )
        }
    }
}

/// Create HTTP response with JSON body
fn create_response(status_code: u16, json: serde_json::Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_string_pretty(&json).unwrap();
    let bytes = body.into_bytes();
    
    tiny_http::Response::from_data(bytes)
        .with_status_code(tiny_http::StatusCode::from(status_code))
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
        )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reading_to_data_conversion() {
        let reading = GaugeReading {
            site_code: "05568500".to_string(),
            site_name: "Kingston Mines".to_string(),
            parameter_code: "00065".to_string(),
            unit: "ft".to_string(),
            value: 15.5,
            datetime: "2024-05-01T12:00:00.000-05:00".to_string(),
            qualifier: "P".to_string(),
        };
        
        let data = reading_to_data(&reading);
        
        assert_eq!(data.value, 15.5);
        assert_eq!(data.unit, "ft");
        assert_eq!(data.qualifier, "P");
    }
}

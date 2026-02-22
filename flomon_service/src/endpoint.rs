/// Zone-based HTTP endpoint for flood monitoring
///
/// Provides REST API organized by hydrological zones with lead times
/// and geographic context for flood forecasting.
///
/// ## NEW Zone-Based Endpoints:
/// - GET /zones - List all zones with metadata
/// - GET /zone/{zone_id} - Get all sensors in a zone with current readings
/// - GET /status - Overall basin flood status across all zones
/// - GET /backwater - Backwater flood analysis (Zone 0 + Zone 1)
/// - GET /forecast - Lead time forecast based on active zones
/// - GET /health - Service health check
///
/// ## DEPRECATED Endpoints (still functional but use zone-based views instead):
/// - GET /site/{site_code} - Returns single-site data (use /zone/{zone_id} instead)

use crate::analysis::groupings::group_by_zone;
use crate::zones::{self, ZoneMetadata, get_zone, get_all_zones};
use crate::model::GaugeReading;
use chrono::{DateTime, Utc};
use postgres::Client;
use serde::Serialize;

// ============================================================================
// Response Types
// ============================================================================

/// List of all zones with metadata
#[derive(Debug, Serialize)]
pub struct ZonesListResponse {
    pub zones: Vec<ZoneListItem>,
    pub system_time: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ZoneListItem {
    pub zone_id: usize,
    pub name: String,
    pub lead_time_hours_min: Option<i64>,
    pub lead_time_hours_max: Option<i64>,
    pub primary_alert_condition: String,
    pub sensor_count: usize,
}

/// Zone detail with all sensor readings
#[derive(Debug, Serialize)]
pub struct ZoneDetailResponse {
    pub zone_id: usize,
    pub zone_name: String,
    pub description: String,
    pub metadata: ZoneMetadataResponse,
    pub sensors: Vec<SensorDetailResponse>,
    pub zone_status: ZoneStatusResponse,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ZoneMetadataResponse {
    pub lead_time_hours_min: Option<i64>,
    pub lead_time_hours_max: Option<i64>,
    pub primary_alert_condition: String,
}

#[derive(Debug, Serialize)]
pub struct SensorDetailResponse {
    pub sensor_id: String,
    pub sensor_type: String,
    pub role: String,
    pub location: String,
    pub coordinates: CoordinatesResponse,
    pub source: String,
    
    // Current readings
    pub current_value: Option<f64>,
    pub current_unit: Option<String>,
    pub current_timestamp: Option<String>,
    pub staleness_minutes: Option<i64>,
    
    // Thresholds (if applicable)
    pub flood_stage_ft: Option<f64>,
    pub action_stage_ft: Option<f64>,
    
    // Relevance explanation
    pub relevance: String,
}

#[derive(Debug, Serialize)]
pub struct CoordinatesResponse {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Serialize)]
pub struct ZoneStatusResponse {
    pub alert_level: String,  // "NORMAL", "WATCH", "WARNING", "CRITICAL"
    pub active_sensors: usize,
    pub stale_sensors: usize,
    pub sensors_above_action: Vec<String>,
    pub sensors_above_flood: Vec<String>,
}

/// Overall basin status
#[derive(Debug, Serialize)]
pub struct BasinStatusResponse {
    pub overall_status: String,  // "NORMAL", "ELEVATED", "FLOOD_WATCH", "FLOOD_WARNING"
    pub active_zones: Vec<ActiveZoneStatus>,
    pub backwater_risk: BackwaterRiskResponse,
    pub upstream_flood_pulse: UpstreamFloodPulseResponse,
    pub compound_event_risk: String,  // "LOW", "MODERATE", "HIGH"
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ActiveZoneStatus {
    pub zone_id: usize,
    pub zone_name: String,
    pub status: String,
    pub lead_time_hours: Option<i64>,
    pub key_sensors_elevated: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BackwaterRiskResponse {
    pub risk_level: String,  // "LOW", "MODERATE", "HIGH", "CRITICAL"
    pub grafton_stage_ft: Option<f64>,
    pub lagrange_pool_ft: Option<f64>,
    pub lagrange_tailwater_ft: Option<f64>,
    pub pool_tailwater_differential_ft: Option<f64>,
    pub explanation: String,
}

#[derive(Debug, Serialize)]
pub struct UpstreamFloodPulseResponse {
    pub pulse_detected: bool,
    pub estimated_arrival_hours: Option<i64>,
    pub source_zones: Vec<usize>,
    pub explanation: String,
}

// ============================================================================
// Main Endpoint Handlers
// ============================================================================

/// Fetch all zones list
pub fn fetch_zones_list(_client: &mut Client) -> Result<ZonesListResponse, String> {
    let zones_config = zones::load_zones_default()
        .map_err(|e| format!("Failed to load zones.toml: {}", e))?;
    
    let mut zone_items = Vec::new();
    
    for (zone_id, zone) in get_all_zones(&zones_config) {
        let metadata = ZoneMetadata::for_zone(zone_id);
        
        zone_items.push(ZoneListItem {
            zone_id,
            name: zone.name.clone(),
            lead_time_hours_min: metadata.lead_time_hours_min,
            lead_time_hours_max: metadata.lead_time_hours_max,
            primary_alert_condition: metadata.primary_alert_condition,
            sensor_count: zone.sensors.len(),
        });
    }
    
    Ok(ZonesListResponse {
        zones: zone_items,
        system_time: Utc::now(),
    })
}

/// Fetch zone detail with all sensor readings
pub fn fetch_zone_detail(client: &mut Client, zone_id: usize) -> Result<ZoneDetailResponse, String> {
    let zones_config = zones::load_zones_default()
        .map_err(|e| format!("Failed to load zones.toml: {}", e))?;
    
    let zone = get_zone(&zones_config, zone_id)
        .ok_or_else(|| format!("Zone {} not found", zone_id))?;
    
    let metadata = ZoneMetadata::for_zone(zone_id);
    
    // Fetch all recent USGS readings
    let usgs_readings = fetch_all_recent_readings(client)?;
    
    // Group by zone
    let zone_readings = group_by_zone(usgs_readings, &zones_config);
    let this_zone_readings = zone_readings.iter()
        .find(|zr| zr.zone_id == zone_id)
        .ok_or_else(|| format!("Zone {} readings not found", zone_id))?;
    
    // Build sensor details
    let mut sensors = Vec::new();
    let mut sensors_above_action = Vec::new();
    let mut sensors_above_flood = Vec::new();
    let mut active_count = 0;
    let mut stale_count = 0;
    
    for sensor_data in &this_zone_readings.sensors {
        let sensor = &sensor_data.sensor;
        
        // Extract current reading
        let (current_value, current_unit, current_timestamp, staleness) = 
            if let Some(ref readings) = sensor_data.readings {
                // Prefer stage over discharge for thresholds
                let stage_reading = readings.stage_ft.as_ref();
                let discharge_reading = readings.discharge_cfs.as_ref();
                
                let reading_opt = stage_reading.or(discharge_reading);
                
                if let Some(reading) = reading_opt {
                    let timestamp = chrono::DateTime::parse_from_rfc3339(&reading.datetime)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc));
                    
                    let staleness_min = timestamp.map(|ts| (Utc::now() - ts).num_minutes());
                    
                    active_count += 1;
                    if staleness_min.unwrap_or(9999) > 120 {
                        stale_count += 1;
                    }
                    
                    (Some(reading.value), Some(reading.unit.clone()), 
                     Some(reading.datetime.clone()), staleness_min)
                } else {
                    stale_count += 1;
                    (None, None, None, None)
                }
            } else {
                // For CWMS/ASOS sensors, fetch from appropriate tables
                let (val, unit, ts, stale) = fetch_sensor_reading(client, sensor)?;
                if val.is_some() {
                    active_count += 1;
                    if stale.unwrap_or(9999) > 120 {
                        stale_count += 1;
                    }
                } else {
                    stale_count += 1;
                }
                (val, unit, ts, stale)
            };
        
        // Check thresholds
        if let (Some(value), Some(action)) = (current_value, sensor.action_stage_ft) {
            if value >= action {
                sensors_above_action.push(sensor.primary_id());
            }
        }
        
        if let (Some(value), Some(flood)) = (current_value, sensor.flood_stage_ft) {
            if value >= flood {
                sensors_above_flood.push(sensor.primary_id());
            }
        }
        
        sensors.push(SensorDetailResponse {
            sensor_id: sensor.primary_id(),
            sensor_type: sensor.sensor_type.clone(),
            role: sensor.role.clone(),
            location: sensor.location.clone(),
            coordinates: CoordinatesResponse {
                lat: sensor.lat,
                lon: sensor.lon,
            },
            source: sensor.source.clone(),
            current_value,
            current_unit,
            current_timestamp,
            staleness_minutes: staleness,
            flood_stage_ft: sensor.flood_stage_ft,
            action_stage_ft: sensor.action_stage_ft,
            relevance: sensor.relevance.clone(),
        });
    }
    
    // Determine zone alert level
    let alert_level = if !sensors_above_flood.is_empty() {
        "CRITICAL"
    } else if !sensors_above_action.is_empty() {
        "WARNING"
    } else if stale_count > sensors.len() / 2 {
        "DEGRADED"
    } else {
        "NORMAL"
    };
    
    Ok(ZoneDetailResponse {
        zone_id,
        zone_name: zone.name.clone(),
        description: zone.description.clone(),
        metadata: ZoneMetadataResponse {
            lead_time_hours_min: metadata.lead_time_hours_min,
            lead_time_hours_max: metadata.lead_time_hours_max,
            primary_alert_condition: metadata.primary_alert_condition,
        },
        sensors,
        zone_status: ZoneStatusResponse {
            alert_level: alert_level.to_string(),
            active_sensors: active_count,
            stale_sensors: stale_count,
            sensors_above_action,
            sensors_above_flood,
        },
        last_updated: Utc::now(),
    })
}

/// Fetch overall basin status
pub fn fetch_basin_status(client: &mut Client) -> Result<BasinStatusResponse, String> {
    let _zones_config = zones::load_zones_default()
        .map_err(|e| format!("Failed to load zones.toml: {}", e))?;
    
    let mut active_zones = Vec::new();
    let mut overall_elevated = false;
    let mut overall_watch = false;
    let mut overall_warning = false;
    
    // Check each zone for activity
    for zone_id in 0..=6 {
        let zone_detail = fetch_zone_detail(client, zone_id)?;
        
        let zone_active = match zone_detail.zone_status.alert_level.as_str() {
            "CRITICAL" => {
                overall_warning = true;
                true
            }
            "WARNING" => {
                overall_watch = true;
                true
            }
            "DEGRADED" | "NORMAL" => false,
            _ => false,
        };
        
        if zone_active || !zone_detail.zone_status.sensors_above_action.is_empty() {
            overall_elevated = true;
            let metadata = ZoneMetadata::for_zone(zone_id);
            
            active_zones.push(ActiveZoneStatus {
                zone_id,
                zone_name: zone_detail.zone_name.clone(),
                status: zone_detail.zone_status.alert_level.clone(),
                lead_time_hours: metadata.lead_time_hours_max,
                key_sensors_elevated: zone_detail.zone_status.sensors_above_action,
            });
        }
    }
    
    // Determine overall status
    let overall_status = if overall_warning {
        "FLOOD_WARNING"
    } else if overall_watch {
        "FLOOD_WATCH"
    } else if overall_elevated {
        "ELEVATED"
    } else {
        "NORMAL"
    };
    
    // Backwater risk analysis
    let backwater_risk = analyze_backwater_risk(client)?;
    
    // Upstream flood pulse detection
    let upstream_pulse = detect_upstream_flood_pulse(&active_zones);
    
    // Compound event risk
    let zone_0_active = active_zones.iter().any(|z| z.zone_id == 0);
    let zone_4_plus_active = active_zones.iter().any(|z| z.zone_id >= 4);
    
    let compound_risk = if zone_0_active && zone_4_plus_active {
        "HIGH"
    } else if zone_0_active || zone_4_plus_active {
        "MODERATE"
    } else {
        "LOW"
    };
    
    Ok(BasinStatusResponse {
        overall_status: overall_status.to_string(),
        active_zones,
        backwater_risk,
        upstream_flood_pulse: upstream_pulse,
        compound_event_risk: compound_risk.to_string(),
        last_updated: Utc::now(),
    })
}

/// Analyze backwater flood risk
fn analyze_backwater_risk(client: &mut Client) -> Result<BackwaterRiskResponse, String> {
    // Fetch key sensors from Zone 0 (Mississippi) and Zone 1 (LaGrange)
    let grafton_stage = fetch_cwms_stage(client, "Grafton", "GRFI2")?;
    let lagrange_pool = fetch_cwms_stage(client, "LaGrange", "IL08P")?;
    let lagrange_tailwater = fetch_cwms_stage(client, "LaGrange", "IL08TW")?;
    
    let differential = match (lagrange_pool, lagrange_tailwater) {
        (Some(pool), Some(tw)) => Some(pool - tw),
        _ => None,
    };
    
    // Analyze risk level
    let risk_level = match (grafton_stage, differential) {
        (Some(grafton), Some(diff)) => {
            if grafton > 25.0 && diff < 0.5 {
                "CRITICAL"
            } else if grafton > 20.0 && diff < 1.0 {
                "HIGH"
            } else if grafton > 18.0 || diff < 2.0 {
                "MODERATE"
            } else {
                "LOW"
            }
        }
        _ => "UNKNOWN",
    };
    
    let explanation = format!(
        "Backwater risk is {} based on Grafton stage ({:.1} ft) and LaGrange pool-tailwater differential ({:.1} ft). \
         When Grafton exceeds 20ft and LaGrange differential drops below 1ft, Mississippi backwater is dominating Illinois River drainage.",
        risk_level,
        grafton_stage.unwrap_or(0.0),
        differential.unwrap_or(99.0)
    );
    
    Ok(BackwaterRiskResponse {
        risk_level: risk_level.to_string(),
        grafton_stage_ft: grafton_stage,
        lagrange_pool_ft: lagrange_pool,
        lagrange_tailwater_ft: lagrange_tailwater,
        pool_tailwater_differential_ft: differential,
        explanation,
    })
}

/// Detect upstream flood pulse
fn detect_upstream_flood_pulse(active_zones: &[ActiveZoneStatus]) -> UpstreamFloodPulseResponse {
    let upstream_active: Vec<usize> = active_zones.iter()
        .filter(|z| z.zone_id >= 4)  // Zones 4, 5, 6
        .map(|z| z.zone_id)
        .collect();
    
    let pulse_detected = !upstream_active.is_empty();
    
    let estimated_arrival = if upstream_active.contains(&6) {
        Some(72)  // 3 days from Chicago
    } else if upstream_active.contains(&5) {
        Some(48)  // 2 days from Dresden Island
    } else if upstream_active.contains(&4) {
        Some(24)  // 1 day from Starved Rock
    } else {
        None
    };
    
    let explanation = if pulse_detected {
        format!(
            "Upstream flood pulse detected in zones: {}. Estimated arrival at property in {} hours.",
            upstream_active.iter().map(|z| z.to_string()).collect::<Vec<_>>().join(", "),
            estimated_arrival.unwrap_or(0)
        )
    } else {
        "No upstream flood pulse detected in upper basin zones.".to_string()
    };
    
    UpstreamFloodPulseResponse {
        pulse_detected,
        estimated_arrival_hours: estimated_arrival,
        source_zones: upstream_active,
        explanation,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Fetch all recent USGS readings (last 4 hours)
fn fetch_all_recent_readings(client: &mut Client) -> Result<Vec<GaugeReading>, String> {
    let rows = client.query(
        "SELECT DISTINCT ON (site_code, parameter_code)
            site_code,
            parameter_code,
            unit,
            value,
            reading_time,
            qualifier
         FROM usgs_raw.gauge_readings
         WHERE reading_time >= NOW() - INTERVAL '4 hours'
         ORDER BY site_code, parameter_code, reading_time DESC",
        &[]
    ).map_err(|e| format!("Failed to fetch recent readings: {}", e))?;
    
    let mut readings = Vec::new();
    
    for row in rows {
        let site_code: String = row.get(0);
        let parameter_code: String = row.get(1);
        let unit: String = row.get(2);
        let value: rust_decimal::Decimal = row.get(3);
        let reading_time: DateTime<Utc> = row.get(4);
        let qualifier: String = row.get(5);
        
        readings.push(GaugeReading {
            site_code: site_code.clone(),
            site_name: site_code.clone(),  // Will be enriched later
            parameter_code,
            unit,
            value: value.to_string().parse().unwrap_or(0.0),
            datetime: reading_time.to_rfc3339(),
            qualifier,
        });
    }
    
    Ok(readings)
}

/// Fetch sensor reading (for CWMS/ASOS sensors)
fn fetch_sensor_reading(
    client: &mut Client,
    sensor: &zones::Sensor
) -> Result<(Option<f64>, Option<String>, Option<String>, Option<i64>), String> {
    
    if sensor.is_cwms() {
        // Query CWMS timeseries table
        if let Some(cwms_loc) = &sensor.cwms_location {
            let rows = client.query(
                "SELECT value, unit, timestamp
                 FROM usace.cwms_timeseries
                 WHERE location_id = $1
                 ORDER BY timestamp DESC
                 LIMIT 1",
                &[cwms_loc]
            ).map_err(|e| format!("CWMS query failed: {}", e))?;
            
            if let Some(row) = rows.first() {
                let value: rust_decimal::Decimal = row.get(0);
                let unit: String = row.get(1);
                let timestamp: DateTime<Utc> = row.get(2);
                let staleness = (Utc::now() - timestamp).num_minutes();
                
                return Ok((
                    Some(value.to_string().parse().unwrap_or(0.0)),
                    Some(unit),
                    Some(timestamp.to_rfc3339()),
                    Some(staleness)
                ));
            }
        }
    } else if sensor.is_asos() {
        // Query ASOS observations table
        if let Some(station_id) = &sensor.station_id {
            let rows = client.query(
                "SELECT precip_1hr_in, observation_time
                 FROM asos_observations
                 WHERE station_id = $1
                 ORDER BY observation_time DESC
                 LIMIT 1",
                &[station_id]
            ).map_err(|e| format!("ASOS query failed: {}", e))?;
            
            if let Some(row) = rows.first() {
                let value_opt: Option<f64> = row.get(0);
                let timestamp: DateTime<Utc> = row.get(1);
                let staleness = (Utc::now() - timestamp).num_minutes();
                
                if let Some(value) = value_opt {
                    return Ok((
                        Some(value),
                        Some("in".to_string()),
                        Some(timestamp.to_rfc3339()),
                        Some(staleness)
                    ));
                }
            }
        }
    }
    
    Ok((None, None, None, None))
}

/// Fetch CWMS stage for a specific location
fn fetch_cwms_stage(client: &mut Client, location_name: &str, _shef_id: &str) -> Result<Option<f64>, String> {
    let rows = client.query(
        "SELECT value
         FROM usace.cwms_timeseries
         WHERE location_id LIKE $1
         ORDER BY timestamp DESC
         LIMIT 1",
        &[&format!("%{}%", location_name)]
    ).map_err(|e| format!("CWMS stage query failed: {}", e))?;
    
    if let Some(row) = rows.first() {
        let value: rust_decimal::Decimal = row.get(0);
        Ok(Some(value.to_string().parse().unwrap_or(0.0)))
    } else {
        Ok(None)
    }
}

// ============================================================================
// HTTP Server
// ============================================================================

/// Start HTTP endpoint server on the specified port
pub fn start_endpoint_server(port: u16, mut client: Client) -> Result<(), String> {
    let server = tiny_http::Server::http(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start HTTP server: {}", e))?;
    
    println!("📡 Zone-based HTTP endpoint listening on http://0.0.0.0:{}", port);
    println!("   NEW ZONE-BASED ENDPOINTS:");
    println!("   GET /zones - List all zones with metadata");
    println!("   GET /zone/{{zone_id}} - Get zone detail (0-6)");
    println!("   GET /status - Overall basin flood status");
    println!("   GET /backwater - Backwater flood analysis");
    println!("   GET /health - Service health check");
    println!("   ");
    println!("   DEPRECATED (but still functional):");
    println!("   GET /site/{{site_code}} - Single-site query (use /zone instead)\n");
    
    for request in server.incoming_requests() {
        let url = request.url();
        
        // Route requests
        let response = if url == "/health" {
            handle_health()
        } else if url == "/zones" {
            handle_zones_list(&mut client)
        } else if url.starts_with("/zone/") {
            let zone_id_str = url.trim_start_matches("/zone/");
            handle_zone_detail(&mut client, zone_id_str)
        } else if url == "/status" {
            handle_basin_status(&mut client)
        } else if url == "/backwater" {
            handle_backwater_analysis(&mut client)
        } else if url.starts_with("/site/") {
            // DEPRECATED endpoint
            handle_deprecated_site_query(&mut client, url)
        } else {
            create_response(
                404,
                serde_json::json!({
                    "error": "Not found",
                    "available_endpoints": {
                        "zones": "/zones",
                        "zone_detail": "/zone/{zone_id}",
                        "basin_status": "/status",
                        "backwater_analysis": "/backwater",
                        "health": "/health",
                        "deprecated_site_query": "/site/{site_code}"
                    }
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
            "version": "0.2.0",
            "endpoint_version": "zone-based"
        })
    )
}

/// Handle /zones endpoint
fn handle_zones_list(client: &mut Client) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match fetch_zones_list(client) {
        Ok(data) => create_response(200, serde_json::to_value(&data).unwrap()),
        Err(e) => create_response(500, serde_json::json!({"error": e})),
    }
}

/// Handle /zone/{zone_id} endpoint
fn handle_zone_detail(client: &mut Client, zone_id_str: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let zone_id: usize = match zone_id_str.parse() {
        Ok(id) if id <= 6 => id,
        _ => return create_response(
            400,
            serde_json::json!({
                "error": "Invalid zone_id. Must be 0-6.",
                "valid_zones": [0, 1, 2, 3, 4, 5, 6]
            })
        ),
    };
    
    match fetch_zone_detail(client, zone_id) {
        Ok(data) => create_response(200, serde_json::to_value(&data).unwrap()),
        Err(e) => create_response(500, serde_json::json!({"error": e})),
    }
}

/// Handle /status endpoint
fn handle_basin_status(client: &mut Client) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match fetch_basin_status(client) {
        Ok(data) => create_response(200, serde_json::to_value(&data).unwrap()),
        Err(e) => create_response(500, serde_json::json!({"error": e})),
    }
}

/// Handle /backwater endpoint
fn handle_backwater_analysis(client: &mut Client) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match analyze_backwater_risk(client) {
        Ok(data) => create_response(200, serde_json::to_value(&data).unwrap()),
        Err(e) => create_response(500, serde_json::json!({"error": e})),
    }
}

/// Handle deprecated /site/{site_code} endpoint
fn handle_deprecated_site_query(_client: &mut Client, url: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    create_response(
        410,  // Gone
        serde_json::json!({
            "error": "This endpoint is deprecated",
            "message": "The /site/{site_code} endpoint has been replaced by zone-based views",
            "migration_guide": {
                "instead_of": format!("GET {}", url),
                "use": "GET /zones - to list all zones, then GET /zone/{zone_id} for zone detail",
                "example": "GET /zone/2 - for your property zone (Upper Peoria Lake)"
            },
            "new_endpoints": {
                "zones_list": "/zones",
                "zone_detail": "/zone/{zone_id}",
                "basin_status": "/status",
                "backwater": "/backwater"
            }
        })
    )
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

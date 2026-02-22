/// IEM (Iowa Environmental Mesonet) Data API Client
///
/// Retrieves ASOS (Automated Surface Observing System) station data from
/// the Iowa State University Mesonet API for precipitation and meteorological
/// monitoring relevant to flood prediction.
///
/// API Documentation: https://mesonet.agron.iastate.edu/request/download.phtml
/// Current conditions: https://mesonet.agron.iastate.edu/json/current.py

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

const IEM_BASE_URL: &str = "https://mesonet.agron.iastate.edu";

// ============================================================================
// IEM API Response Structures
// ============================================================================

/// Current observation response from IEM
#[derive(Debug, Deserialize)]
pub struct IemCurrentResponse {
    pub data: Vec<IemObservation>,
}

/// Single weather observation
#[derive(Debug, Deserialize)]
pub struct IemObservation {
    pub station: String,
    pub valid: String,  // ISO 8601 timestamp
    #[serde(rename = "tmpf")]
    pub temp_f: Option<f64>,
    #[serde(rename = "dwpf")]
    pub dewpoint_f: Option<f64>,
    #[serde(rename = "relh")]
    pub relative_humidity: Option<f64>,
    pub drct: Option<f64>,  // Wind direction (degrees)
    pub sknt: Option<f64>,  // Wind speed (knots)
    #[serde(rename = "p01i")]
    pub precip_1hr_in: Option<f64>,  // 1-hour precipitation (inches)
    pub alti: Option<f64>,  // Altimeter setting (inches Hg)
    #[serde(rename = "mslp")]
    pub sea_level_pressure_mb: Option<f64>,
    pub vsby: Option<f64>,  // Visibility (miles)
    #[serde(rename = "gust")]
    pub wind_gust_knots: Option<f64>,
    pub skyc1: Option<String>,  // Sky condition
    pub wxcodes: Option<String>,  // Weather codes (RA, SN, etc.)
}

/// 1-minute ASOS data response (for detailed precip tracking)
#[derive(Debug, Deserialize)]
pub struct IemAsosMinuteResponse {
    pub data: Vec<IemAsosMinute>,
}

#[derive(Debug, Deserialize)]
pub struct IemAsosMinute {
    pub station: String,
    pub valid: String,
    #[serde(rename = "tmpf")]
    pub temp_f: Option<f64>,
    #[serde(rename = "dwpf")]
    pub dewpoint_f: Option<f64>,
    pub sknt: Option<f64>,
    pub drct: Option<f64>,
    #[serde(rename = "p01m")]
    pub precip_1min_in: Option<f64>,  // 1-minute precipitation
}

/// Processed observation for database storage
#[derive(Debug, Clone)]
pub struct AsosObservation {
    pub station_id: String,
    pub timestamp: DateTime<Utc>,
    pub temp_f: Option<f64>,
    pub dewpoint_f: Option<f64>,
    pub relative_humidity: Option<f64>,
    pub wind_direction_deg: Option<f64>,
    pub wind_speed_knots: Option<f64>,
    pub wind_gust_knots: Option<f64>,
    pub precip_1hr_in: Option<f64>,
    pub pressure_mb: Option<f64>,
    pub visibility_mi: Option<f64>,
    pub sky_condition: Option<String>,
    pub weather_codes: Option<String>,
}

// ============================================================================
// API Client Functions
// ============================================================================

/// Fetch current observations for a station
///
/// # Parameters
/// - `client`: HTTP client
/// - `station_id`: ASOS station ID (e.g., "KPIA")
///
/// # Returns
/// Latest observation for the station
pub fn fetch_current(
    client: &reqwest::blocking::Client,
    station_id: &str,
) -> Result<AsosObservation, Box<dyn std::error::Error>> {
    
    let url = format!(
        "{}/json/current.py?station={}",
        IEM_BASE_URL,
        station_id
    );
    
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()?;
    
    if !response.status().is_success() {
        return Err(format!("IEM API error: {}", response.status()).into());
    }
    
    let api_response: IemCurrentResponse = response.json()?;
    
    let obs = api_response.data.into_iter()
        .next()
        .ok_or("No data returned for station")?;
    
    parse_observation(obs)
}

/// Fetch recent observations (last N hours)
///
/// Uses the ASOS endpoint for comprehensive weather data
pub fn fetch_recent_precip(
    client: &reqwest::blocking::Client,
    station_id: &str,
    hours: i64,
) -> Result<Vec<AsosObservation>, Box<dyn std::error::Error>> {
    
    let end = Utc::now();
    let begin = end - chrono::Duration::hours(hours);
    
    let url = format!(
        "{}/cgi-bin/request/asos.py?station={}&data=all&year1={}&month1={}&day1={}&hour1={}&year2={}&month2={}&day2={}&hour2={}&tz=UTC&format=onlycomma&latlon=no&elev=no&missing=null&trace=null&direct=no",
        IEM_BASE_URL,
        station_id,
        begin.format("%Y"),
        begin.format("%m"),
        begin.format("%d"),
        begin.format("%H"),
        end.format("%Y"),
        end.format("%m"),
        end.format("%d"),
        end.format("%H")
    );
    
    let response = client
        .get(&url)
        .send()?;
    
    if !response.status().is_success() {
        return Err(format!("IEM ASOS API error: {}", response.status()).into());
    }
    
    let text = response.text()?;
    parse_asos_csv(&text, station_id)
}

/// Parse IEM ASOS CSV response
fn parse_asos_csv(csv: &str, _station_id: &str) -> Result<Vec<AsosObservation>, Box<dyn std::error::Error>> {
    let mut observations = Vec::new();
    
    for (i, line) in csv.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // Skip header or empty lines
        }
        
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 21 {
            continue;  // Skip incomplete rows
        }
        
        // Helper to parse values that might be "null"
        let parse_field = |s: &str| -> Option<f64> {
            if s.trim() == "null" || s.trim().is_empty() {
                None
            } else {
                s.trim().parse().ok()
            }
        };
        
        // Parse timestamp (format: "2026-02-21 19:54")
        let timestamp_str = fields[1];
        let timestamp = NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M")
            .ok()
            .and_then(|dt| Some(DateTime::from_naive_utc_and_offset(dt, Utc)))
            .ok_or("Failed to parse timestamp")?;
        
        let station_id = fields[0].to_string();
        let temp_f = parse_field(fields[2]);
        let dewpoint_f = parse_field(fields[3]);
        let relative_humidity = parse_field(fields[4]);
        let wind_direction_deg = parse_field(fields[5]);
        let wind_speed_knots = parse_field(fields[6]);
        let precip_1hr_in = parse_field(fields[7]);
        let altimeter_in_hg = parse_field(fields[8]);
        let pressure_mb = parse_field(fields[9]);
        let visibility_mi = parse_field(fields[10]);
        let wind_gust_knots = parse_field(fields[11]);
        
        // Sky conditions (combine up to 4 layers)
        let sky_layers: Vec<String> = (12..16)
            .filter_map(|i| {
                let cond = fields[i].trim();
                if cond != "null" && !cond.is_empty() {
                    Some(cond.to_string())
                } else {
                    None
                }
            })
            .collect();
        let sky_condition = if sky_layers.is_empty() {
            None
        } else {
            Some(sky_layers.join("/"))
        };
        
        // Weather codes
        let weather_codes = if fields[20].trim() == "null" || fields[20].trim().is_empty() {
            None
        } else {
            Some(fields[20].to_string())
        };
        
        observations.push(AsosObservation {
            station_id,
            timestamp,
            temp_f,
            dewpoint_f,
            relative_humidity,
            wind_direction_deg,
            wind_speed_knots,
            wind_gust_knots,
            precip_1hr_in,
            pressure_mb,
            visibility_mi,
            sky_condition,
            weather_codes,
        });
    }
    
    Ok(observations)
}

/// Parse a single IEM observation into our format
fn parse_observation(obs: IemObservation) -> Result<AsosObservation, Box<dyn std::error::Error>> {
    // Parse ISO 8601 timestamp
    let timestamp = DateTime::parse_from_rfc3339(&obs.valid)?
        .with_timezone(&Utc);
    
    Ok(AsosObservation {
        station_id: obs.station,
        timestamp,
        temp_f: obs.temp_f,
        dewpoint_f: obs.dewpoint_f,
        relative_humidity: obs.relative_humidity,
        wind_direction_deg: obs.drct,
        wind_speed_knots: obs.sknt,
        wind_gust_knots: obs.wind_gust_knots,
        precip_1hr_in: obs.precip_1hr_in,
        pressure_mb: obs.sea_level_pressure_mb,
        visibility_mi: obs.vsby,
        sky_condition: obs.skyc1,
        weather_codes: obs.wxcodes,
    })
}

// ============================================================================
// Precipitation Analysis Helpers
// ============================================================================

/// Calculate cumulative precipitation over a time period
pub fn calculate_cumulative_precip(observations: &[AsosObservation]) -> f64 {
    observations.iter()
        .filter_map(|obs| obs.precip_1hr_in)
        .sum()
}

/// Detect significant rainfall events (>= threshold inches in period)
pub fn detect_rainfall_event(observations: &[AsosObservation], threshold_in: f64) -> bool {
    calculate_cumulative_precip(observations) >= threshold_in
}

/// Calculate precipitation intensity (inches per hour)
pub fn calculate_precip_intensity(observations: &[AsosObservation], hours: usize) -> Option<f64> {
    if observations.is_empty() || hours == 0 {
        return None;
    }
    
    let total = calculate_cumulative_precip(observations);
    Some(total / hours as f64)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cumulative_precip() {
        let obs = vec![
            AsosObservation {
                station_id: "KPIA".to_string(),
                timestamp: Utc::now(),
                temp_f: Some(65.0),
                dewpoint_f: Some(55.0),
                relative_humidity: None,
                wind_direction_deg: Some(180.0),
                wind_speed_knots: Some(10.0),
                wind_gust_knots: None,
                precip_1hr_in: Some(0.25),
                pressure_mb: Some(1013.0),
                visibility_mi: Some(10.0),
                sky_condition: None,
                weather_codes: None,
            },
            AsosObservation {
                station_id: "KPIA".to_string(),
                timestamp: Utc::now(),
                temp_f: Some(64.0),
                dewpoint_f: Some(54.0),
                relative_humidity: None,
                wind_direction_deg: Some(190.0),
                wind_speed_knots: Some(12.0),
                wind_gust_knots: None,
                precip_1hr_in: Some(0.30),
                pressure_mb: Some(1012.0),
                visibility_mi: Some(8.0),
                sky_condition: None,
                weather_codes: None,
            },
        ];
        
        assert_eq!(calculate_cumulative_precip(&obs), 0.55);
    }
    
    #[test]
    fn test_detect_rainfall_event() {
        let obs = vec![
            AsosObservation {
                station_id: "KPIA".to_string(),
                timestamp: Utc::now(),
                temp_f: None,
                dewpoint_f: None,
                relative_humidity: None,
                wind_direction_deg: None,
                wind_speed_knots: None,
                wind_gust_knots: None,
                precip_1hr_in: Some(0.75),
                pressure_mb: None,
                visibility_mi: None,
                sky_condition: None,
                weather_codes: None,
            },
        ];
        
        assert!(detect_rainfall_event(&obs, 0.5));
        assert!(!detect_rainfall_event(&obs, 1.0));
    }
}

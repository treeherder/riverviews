/// GaugeReading, SiteReadings, FloodThresholds, NwisError
/// core data structures and error handling
/// 
/// maybe also the "is this a valid site code" test, since that's pretty fundamental to the data model
/// the site code format test (test_peoria_basin_site_codes_are_valid_format) is validating the shape of our data identifiers.
/// Core data types for the Peoria flood monitoring service.
///
/// This module defines the shared domain model imported by all other modules.
/// It contains no logic, no I/O, and no external dependencies — only types.

// ---------------------------------------------------------------------------
// Parameter codes
// ---------------------------------------------------------------------------

/// USGS parameter code for discharge (streamflow), in cubic feet per second.
pub const PARAM_DISCHARGE: &str = "00060";

/// USGS parameter code for gage height (stage), in feet.
pub const PARAM_STAGE: &str = "00065";

// ---------------------------------------------------------------------------
// Reading types
// ---------------------------------------------------------------------------

/// A single instantaneous measurement from a USGS gauge station.
///
/// Corresponds to one entry in the `values[].value[]` array of a USGS
/// IV API response, enriched with site and parameter metadata from the
/// enclosing `timeSeries` object.
#[derive(Debug, Clone, PartialEq)]
pub struct GaugeReading {
    pub site_code: String,
    pub site_name: String,
    pub parameter_code: String,
    pub unit: String,
    pub value: f64,
    pub datetime: String,   // ISO 8601, e.g. "2024-05-01T12:00:00.000-05:00"
    pub qualifier: String,  // "P" = provisional, "A" = approved
}

/// Both available readings for a single site, grouped for convenient access.
///
/// Produced by `analysis::grouping::group_by_site` from a flat list of
/// `GaugeReading`s. Either field may be `None` if the site does not report
/// that parameter or if the reading was unavailable.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteReadings {
    pub site_code: String,
    pub discharge_cfs: Option<GaugeReading>, // param 00060
    pub stage_ft: Option<GaugeReading>,      // param 00065
}

// ---------------------------------------------------------------------------
// Threshold types
// ---------------------------------------------------------------------------

/// Official NWS flood stage thresholds for a gauge station, in feet.
///
/// Thresholds are sourced from the NWS Advanced Hydrologic Prediction
/// Service (AHPS) and stored in `stations::STATION_REGISTRY`.
///
/// Stage levels in ascending order:
///   action < flood < moderate_flood < major_flood
#[derive(Debug, Clone)]
pub struct FloodThresholds {
    pub action_stage_ft: f64,
    pub flood_stage_ft: f64,
    pub moderate_flood_stage_ft: f64,
    pub major_flood_stage_ft: f64,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can arise when fetching or processing USGS NWIS data.
#[derive(Debug, PartialEq)]
pub enum NwisError {
    /// Non-2xx HTTP response from the USGS API.
    HttpError(u16),
    /// The response body could not be deserialized.
    ParseError(String),
    /// The requested site code was not found in the response.
    SiteNotFound(String),
    /// The site was found but contained no usable data values
    /// (empty array or sentinel -999999).
    NoDataAvailable(String),
    /// A reading exists but is older than the configured freshness threshold.
    StaleData { site: String, age_minutes: u64 },
}

impl std::fmt::Display for NwisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NwisError::HttpError(code) => write!(f, "HTTP error: {}", code),
            NwisError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            NwisError::SiteNotFound(site) => write!(f, "Site not found: {}", site),
            NwisError::NoDataAvailable(site) => write!(f, "No data available for site: {}", site),
            NwisError::StaleData { site, age_minutes } => {
                write!(f, "Stale data for site {}: {} minutes old", site, age_minutes)
            }
        }
    }
}

impl std::error::Error for NwisError {}

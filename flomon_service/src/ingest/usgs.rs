/// build_iv_url, parse_iv_response + those tests
/// When we add NWS or MRMS data sources later, they each get their own file under ingest/ rather than bloating this one.
/// /// USGS NWIS Instantaneous Values (IV) API client.
///
/// Handles URL construction and JSON response parsing for the USGS Water
/// Services IV endpoint:
///   https://waterservices.usgs.gov/nwis/iv/
///
/// The IV service returns WaterML rendered as JSON. See `fixtures.rs` for
/// annotated examples of the response structure.

use crate::model::{GaugeReading, NwisError};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Serde structures for WaterML JSON deserialization
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct IvResponse {
    value: ValueWrapper,
}

#[derive(Deserialize)]
struct ValueWrapper {
    #[serde(rename = "timeSeries")]
    time_series: Vec<TimeSeries>,
}

#[derive(Deserialize)]
struct TimeSeries {
    #[serde(rename = "sourceInfo")]
    source_info: SourceInfo,
    variable: Variable,
    values: Vec<Values>,
}

#[derive(Deserialize)]
struct SourceInfo {
    #[serde(rename = "siteName")]
    site_name: String,
    #[serde(rename = "siteCode")]
    site_code: Vec<SiteCode>,
}

#[derive(Deserialize)]
struct SiteCode {
    value: String,
}

#[derive(Deserialize)]
struct Variable {
    #[serde(rename = "variableCode")]
    variable_code: Vec<VariableCode>,
    unit: Unit,
    #[serde(rename = "noDataValue")]
    no_data_value: f64,
}

#[derive(Deserialize)]
struct VariableCode {
    value: String,
}

#[derive(Deserialize)]
struct Unit {
    #[serde(rename = "unitCode")]
    unit_code: String,
}

#[derive(Deserialize)]
struct Values {
    value: Vec<ValueEntry>,
}

#[derive(Deserialize)]
struct ValueEntry {
    value: String,  // USGS returns as string!
    qualifiers: Vec<String>,
    #[serde(rename = "dateTime")]
    date_time: String,
}

// ---------------------------------------------------------------------------
// URL construction
// ---------------------------------------------------------------------------

const IV_BASE_URL: &str = "https://waterservices.usgs.gov/nwis/iv/";
const DV_BASE_URL: &str = "https://waterservices.usgs.gov/nwis/dv/";

/// Builds a USGS IV API URL for the given site codes, parameter codes,
/// and ISO 8601 period (e.g. `"PT1H"` for the past hour, `"PT3H"` for
/// the past three hours).
///
/// The returned URL always requests JSON format and filters to active
/// sites only.
///
/// # Example
/// ```
/// use flomon_service::ingest::usgs::build_iv_url;
/// use flomon_service::stations::{PARAM_DISCHARGE, PARAM_STAGE};
/// 
/// // Request data from Kingston Mines and Peoria stations
/// let url = build_iv_url(
///     &["05568500", "05567500"],  // Site codes from STATION_REGISTRY
///     &[PARAM_DISCHARGE, PARAM_STAGE],
///     "PT3H",
/// );
/// ```
pub fn build_iv_url(sites: &[&str], param_codes: &[&str], period: &str) -> String {
    let sites_param = sites.join(",");
    let params_param = param_codes.join(",");
    let format_param = "json";
    let site_status = "active";
    
    format!(
        "{}?sites={}&parameterCd={}&period={}&format={}&siteStatus={}",
        IV_BASE_URL,
        sites_param,
        params_param,
        period,
        format_param,
        site_status
    )
}

/// Builds a USGS Daily Values (DV) API URL for the given site codes,
/// parameter codes, and date range.
///
/// Unlike the IV API which uses ISO 8601 periods, the DV API uses
/// explicit start and end dates in YYYY-MM-DD format.
///
/// # Example
/// ```
/// use flomon_service::ingest::usgs::build_dv_url;
/// use flomon_service::stations::{PARAM_DISCHARGE, PARAM_STAGE};
/// 
/// // Request historical daily data
/// let url = build_dv_url(
///     &["05568500"],
///     &[PARAM_DISCHARGE, PARAM_STAGE],
///     "2020-01-01",
///     "2020-12-31",
/// );
/// ```
pub fn build_dv_url(
    sites: &[&str],
    param_codes: &[&str],
    start_date: &str,
    end_date: &str,
) -> String {
    let sites_param = sites.join(",");
    let params_param = param_codes.join(",");
    
    format!(
        "{}?sites={}&parameterCd={}&startDT={}&endDT={}&format=json",
        DV_BASE_URL,
        sites_param,
        params_param,
        start_date,
        end_date
    )
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parses a USGS IV API JSON response body into a flat list of
/// `GaugeReading`s, one per `timeSeries` entry that contains valid data.
///
/// # Errors
/// - `NwisError::ParseError` — malformed or unexpected JSON structure.
/// - `NwisError::NoDataAvailable` — all `timeSeries` entries had either
///   an empty `value` array or the USGS sentinel value (`-999999`).
pub fn parse_iv_response(json: &str) -> Result<Vec<GaugeReading>, NwisError> {
    // Parse the JSON into our serde structs
    let response: IvResponse = serde_json::from_str(json)
        .map_err(|e| NwisError::ParseError(format!("JSON deserialization failed: {}", e)))?;

    // Check for empty timeSeries array
    if response.value.time_series.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "No timeSeries entries in response".to_string(),
        ));
    }

    let mut readings = Vec::new();

    // Process each timeSeries entry
    for series in response.value.time_series {
        // Extract metadata
        let site_code = series
            .source_info
            .site_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing siteCode".to_string()))?
            .value
            .clone();

        let site_name = series.source_info.site_name.clone();

        let parameter_code = series
            .variable
            .variable_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing variableCode".to_string()))?
            .value
            .clone();

        let unit = series.variable.unit.unit_code.clone();
        let no_data_value = series.variable.no_data_value;

        // Get the values array
        let values_wrapper = series
            .values
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing values array".to_string()))?;

        // Check for empty value array
        if values_wrapper.value.is_empty() {
            continue; // Skip this series, try others
        }

        // Get the most recent value (last entry in chronologically sorted array)
        let latest = values_wrapper
            .value
            .last()
            .ok_or_else(|| NwisError::ParseError("Empty value array".to_string()))?;

        // Parse the value string to f64
        let value: f64 = latest
            .value
            .parse()
            .map_err(|e| NwisError::ParseError(format!("Failed to parse value '{}': {}", latest.value, e)))?;

        // Check for sentinel value
        if (value - no_data_value).abs() < 0.1 {
            continue; // Skip this series, try others
        }

        // Get qualifier, defaulting to "P" if not present
        let qualifier = latest
            .qualifiers
            .first()
            .map(|s| s.as_str())
            .unwrap_or("P")
            .to_string();

        // Create the GaugeReading
        readings.push(GaugeReading {
            site_code,
            site_name,
            parameter_code,
            unit,
            value,
            datetime: latest.date_time.clone(),
            qualifier,
        });
    }

    // If we didn't collect any valid readings, return NoDataAvailable
    if readings.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "All timeSeries entries were empty or contained sentinel values".to_string(),
        ));
    }

    Ok(readings)
}

/// Parses a USGS IV API JSON response into ALL readings (not just latest).
/// Similar to parse_dv_response but for instantaneous values.
/// Use this for backfilling gaps with high-resolution data.
pub fn parse_iv_response_all(json: &str) -> Result<Vec<GaugeReading>, NwisError> {
    let response: IvResponse = serde_json::from_str(json)
        .map_err(|e| NwisError::ParseError(format!("JSON deserialization failed: {}", e)))?;

    if response.value.time_series.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "No timeSeries entries in response".to_string(),
        ));
    }

    let mut all_readings = Vec::new();

    for series in response.value.time_series {
        let site_code = series
            .source_info
            .site_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing siteCode".to_string()))?
            .value
            .clone();

        let site_name = series.source_info.site_name.clone();

        let parameter_code = series
            .variable
            .variable_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing variableCode".to_string()))?
            .value
            .clone();

        let unit = series.variable.unit.unit_code.clone();
        let no_data_value = series.variable.no_data_value;

        let values_wrapper = series
            .values
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing values array".to_string()))?;

        if values_wrapper.value.is_empty() {
            continue;
        }

        // Process ALL values (not just the most recent)
        for entry in &values_wrapper.value {
            let value: f64 = match entry.value.parse() {
                Ok(v) => v,
                Err(_) => continue, // Skip unparseable values
            };

            // Skip sentinel values
            if (value - no_data_value).abs() < 0.1 {
                continue;
            }

            let qualifier = entry
                .qualifiers
                .first()
                .map(|s| s.as_str())
                .unwrap_or("P")
                .to_string();

            all_readings.push(GaugeReading {
                site_code: site_code.clone(),
                site_name: site_name.clone(),
                parameter_code: parameter_code.clone(),
                unit: unit.clone(),
                value,
                datetime: entry.date_time.clone(),
                qualifier: qualifier.clone(),
            });
        }
    }

    if all_readings.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "All timeSeries entries were empty or contained sentinel values".to_string(),
        ));
    }

    Ok(all_readings)
}

/// Parses a USGS Daily Values (DV) API JSON response into a flat list
/// of `GaugeReading`s, returning ALL daily values in the time range.
///
/// Unlike `parse_iv_response` which returns only the most recent value
/// per timeSeries, this returns one reading per day for historical analysis.
///
/// # Errors
/// - `NwisError::ParseError` — malformed or unexpected JSON structure.
/// - `NwisError::NoDataAvailable` — all `timeSeries` entries had either
///   an empty `value` array or only USGS sentinel values (`-999999`).
pub fn parse_dv_response(json: &str) -> Result<Vec<GaugeReading>, NwisError> {
    // Parse the JSON into our serde structs (same format as IV)
    let response: IvResponse = serde_json::from_str(json)
        .map_err(|e| NwisError::ParseError(format!("JSON deserialization failed: {}", e)))?;

    // Check for empty timeSeries array
    if response.value.time_series.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "No timeSeries entries in response".to_string(),
        ));
    }

    let mut all_readings = Vec::new();

    // Process each timeSeries entry
    for series in response.value.time_series {
        // Extract metadata
        let site_code = series
            .source_info
            .site_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing siteCode".to_string()))?
            .value
            .clone();

        let site_name = series.source_info.site_name.clone();

        let parameter_code = series
            .variable
            .variable_code
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing variableCode".to_string()))?
            .value
            .clone();

        let unit = series.variable.unit.unit_code.clone();
        let no_data_value = series.variable.no_data_value;

        // Get the values array
        let values_wrapper = series
            .values
            .first()
            .ok_or_else(|| NwisError::ParseError("Missing values array".to_string()))?;

        // Check for empty value array
        if values_wrapper.value.is_empty() {
            continue; // Skip this series, try others
        }

        // Process ALL values (not just the most recent like IV does)
        for entry in &values_wrapper.value {
            // Parse the value string to f64
            let value: f64 = match entry.value.parse() {
                Ok(v) => v,
                Err(e) => {
                    // Log but don't fail - skip bad values
                    eprintln!("Warning: Failed to parse value '{}': {}", entry.value, e);
                    continue;
                }
            };

            // Skip sentinel values
            if (value - no_data_value).abs() < 0.1 {
                continue;
            }

            // Get qualifier, defaulting to "P" if not present
            let qualifier = entry
                .qualifiers
                .first()
                .map(|s| s.as_str())
                .unwrap_or("P")
                .to_string();

            // Create a GaugeReading for each daily value
            all_readings.push(GaugeReading {
                site_code: site_code.clone(),
                site_name: site_name.clone(),
                parameter_code: parameter_code.clone(),
                unit: unit.clone(),
                value,
                datetime: entry.date_time.clone(),
                qualifier: qualifier.clone(),
            });
        }
    }

    // If we didn't collect any valid readings, return NoDataAvailable
    if all_readings.is_empty() {
        return Err(NwisError::NoDataAvailable(
            "All timeSeries entries were empty or contained sentinel values".to_string(),
        ));
    }

    Ok(all_readings)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::fixtures::*;
    use crate::stations::{all_site_codes, PARAM_DISCHARGE, PARAM_STAGE};

    // --- URL construction ---------------------------------------------------

    #[test]
    fn test_build_url_targets_iv_endpoint_with_json_format() {
        let url = build_iv_url(&["05568500"], &[PARAM_DISCHARGE, PARAM_STAGE], "PT3H");
        assert!(
            url.contains("waterservices.usgs.gov/nwis/iv/"),
            "must target the IV endpoint, got: {}",
            url
        );
        assert!(url.contains("format=json"), "must request JSON format");
    }

    #[test]
    fn test_build_url_includes_all_params() {
        let url = build_iv_url(&["05568500"], &[PARAM_DISCHARGE, PARAM_STAGE], "PT3H");
        assert!(url.contains("05568500"), "must include site code");
        assert!(url.contains(PARAM_DISCHARGE), "must include discharge param");
        assert!(url.contains(PARAM_STAGE), "must include stage param");
        assert!(url.contains("PT3H"), "must include ISO 8601 period");
        assert!(url.contains("siteStatus=active"), "should filter to active sites");
    }

    #[test]
    fn test_build_url_with_all_peoria_basin_sites() {
        let sites = all_site_codes();
        let site_refs: Vec<&str> = sites.iter().map(|s| s.as_str()).collect();
        let url = build_iv_url(&site_refs, &[PARAM_DISCHARGE, PARAM_STAGE], "PT1H");
        for site in &sites {
            assert!(url.contains(site), "URL must include site {}", site);
        }
    }

    #[test]
    fn test_build_url_uses_comma_separated_sites() {
        let url = build_iv_url(&["05568500", "05567500"], &[PARAM_DISCHARGE], "PT1H");
        // USGS expects a single comma-separated `sites` param, not repeated params.
        assert!(
            url.contains("05568500,05567500") || url.contains("sites=05568500&sites=05567500"),
            "sites should be comma-separated, got: {}",
            url
        );
    }

    // --- DV URL construction ------------------------------------------------

    #[test]
    fn test_build_dv_url_targets_dv_endpoint() {
        let url = build_dv_url(
            &["05568500"],
            &[PARAM_DISCHARGE, PARAM_STAGE],
            "2020-01-01",
            "2020-12-31",
        );
        assert!(
            url.contains("waterservices.usgs.gov/nwis/dv/"),
            "must target the DV endpoint, got: {}",
            url
        );
        assert!(url.contains("format=json"), "must request JSON format");
    }

    #[test]
    fn test_build_dv_url_includes_date_range() {
        let url = build_dv_url(
            &["05568500"],
            &[PARAM_DISCHARGE],
            "2020-06-01",
            "2020-06-30",
        );
        assert!(url.contains("startDT=2020-06-01"), "must include start date");
        assert!(url.contains("endDT=2020-06-30"), "must include end date");
        assert!(url.contains("05568500"), "must include site code");
        assert!(url.contains(PARAM_DISCHARGE), "must include parameter code");
    }

    #[test]
    fn test_build_dv_url_historical_range() {
        let url = build_dv_url(
            &["05568500"],
            &[PARAM_DISCHARGE, PARAM_STAGE],
            "1939-10-01",
            "1940-09-30",
        );
        assert!(url.contains("1939-10-01"), "must support earliest data year");
        assert!(url.contains("1940-09-30"), "must support full year range");
    }

    // --- Parsing: happy path ------------------------------------------------

    #[test]
    fn test_parse_kingston_mines_discharge_value_and_metadata() {
        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("valid fixture should parse without error");

        let discharge = readings
            .iter()
            .find(|r| r.site_code == "05568500" && r.parameter_code == "00060")
            .expect("should find discharge reading for Kingston Mines");

        assert_eq!(discharge.site_name, "Illinois River at Kingston Mines, IL");
        assert_eq!(discharge.unit, "ft3/s");
        assert!(
            (discharge.value - 42_300.0).abs() < 0.01,
            "discharge should be 42300 cfs, got {}",
            discharge.value
        );
        assert_eq!(discharge.qualifier, "P");
        assert!(
            discharge.datetime.starts_with("2024-05-01"),
            "datetime should be parsed correctly, got {}",
            discharge.datetime
        );
    }

    #[test]
    fn test_parse_kingston_mines_stage_value_and_unit() {
        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("valid fixture should parse");

        let stage = readings
            .iter()
            .find(|r| r.site_code == "05568500" && r.parameter_code == "00065")
            .expect("should find stage reading for Kingston Mines");

        assert_eq!(stage.unit, "ft");
        assert!(
            (stage.value - 18.42).abs() < 0.001,
            "stage should be 18.42 ft, got {}",
            stage.value
        );
    }

    #[test]
    fn test_parse_single_site_returns_one_reading_per_parameter() {
        let readings = parse_iv_response(fixture_kingston_mines_json())
            .expect("should parse");

        let for_site: Vec<_> = readings
            .iter()
            .filter(|r| r.site_code == "05568500")
            .collect();

        assert_eq!(
            for_site.len(),
            2,
            "should return exactly one reading per parameter (discharge + stage)"
        );
        assert!(for_site.iter().any(|r| r.parameter_code == "00060"), "should include discharge");
        assert!(for_site.iter().any(|r| r.parameter_code == "00065"), "should include stage");
    }

    #[test]
    fn test_parse_multi_site_response_returns_reading_for_each_site() {
        let readings = parse_iv_response(fixture_multi_site_json())
            .expect("multi-site fixture should parse");

        let peoria = readings
            .iter()
            .find(|r| r.site_code == "05567500")
            .expect("should include Peoria pool gauge");
        assert!(
            (peoria.value - 14.85).abs() < 0.001,
            "Peoria stage should be 14.85 ft"
        );

        let chillicothe = readings
            .iter()
            .find(|r| r.site_code == "05568000")
            .expect("should include Chillicothe gauge");
        assert!(
            (chillicothe.value - 39_100.0).abs() < 0.1,
            "Chillicothe discharge should be 39100 cfs"
        );
    }

    #[test]
    fn test_parse_approved_qualifier_is_preserved() {
        let readings = parse_iv_response(fixture_approved_qualifier_json())
            .expect("approved data fixture should parse");

        let reading = readings.first().expect("should have at least one reading");
        assert_eq!(
            reading.qualifier, "A",
            "qualifier should be 'A' for approved data"
        );
    }

    // --- Parsing: error and edge cases --------------------------------------

    #[test]
    fn test_parse_empty_value_array_returns_no_data_available() {
        let result = parse_iv_response(fixture_empty_value_array_json());
        assert!(
            matches!(result, Err(NwisError::NoDataAvailable(_))),
            "empty value array should yield NoDataAvailable, got {:?}",
            result
        );
    }

    #[test]
    fn test_parse_sentinel_value_returns_no_data_available() {
        // USGS uses the string "-999999" as a sentinel even when a timestamp
        // is present. This must not be stored as a valid reading.
        let result = parse_iv_response(fixture_sentinel_no_data_json());
        assert!(
            matches!(result, Err(NwisError::NoDataAvailable(_))),
            "sentinel value -999999 should yield NoDataAvailable, got {:?}",
            result
        );
    }

    #[test]
    fn test_parse_malformed_json_returns_parse_error() {
        let result = parse_iv_response("{ this is not valid json }}}");
        assert!(
            matches!(result, Err(NwisError::ParseError(_))),
            "malformed JSON should return ParseError, got {:?}",
            result
        );
    }

    #[test]
    fn test_parse_empty_string_returns_parse_error() {
        let result = parse_iv_response("");
        assert!(
            matches!(result, Err(NwisError::ParseError(_))),
            "empty input should return ParseError"
        );
    }

    #[test]
    fn test_parse_empty_time_series_array_returns_no_data() {
        let json = r#"{ "value": { "timeSeries": [] } }"#;
        let result = parse_iv_response(json);
        assert!(
            matches!(result, Err(NwisError::NoDataAvailable(_))),
            "empty timeSeries should yield NoDataAvailable"
        );
    }

    #[test]
    fn test_parse_missing_values_field_returns_error() {
        // Structurally valid JSON envelope but the `values` array is absent
        // from the timeSeries entry — defensive against unexpected API changes.
        let json = r#"{
          "value": {
            "timeSeries": [{
              "sourceInfo": {
                "siteName": "Test Site",
                "siteCode": [{ "value": "99999999", "network": "NWIS" }]
              },
              "variable": {
                "variableCode": [{ "value": "00060", "network": "NWIS" }],
                "variableName": "Streamflow",
                "unit": { "unitCode": "ft3/s" },
                "noDataValue": -999999.0
              }
            }]
          }
        }"#;
        let result = parse_iv_response(json);
        assert!(
            matches!(
                result,
                Err(NwisError::ParseError(_)) | Err(NwisError::NoDataAvailable(_))
            ),
            "missing values field should return an error, got {:?}",
            result
        );
    }
}

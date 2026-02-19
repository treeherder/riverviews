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

// ---------------------------------------------------------------------------
// URL construction
// ---------------------------------------------------------------------------

const IV_BASE_URL: &str = "https://waterservices.usgs.gov/nwis/iv/";

/// Builds a USGS IV API URL for the given site codes, parameter codes,
/// and ISO 8601 period (e.g. `"PT1H"` for the past hour, `"PT3H"` for
/// the past three hours).
///
/// The returned URL always requests JSON format and filters to active
/// sites only.
///
/// # Example
/// ```
/// let url = build_iv_url(
///     &["05568500", "05567500"],
///     &["00060", "00065"],
///     "PT3H",
/// );
/// ```
pub fn build_iv_url(sites: &[&str], param_codes: &[&str], period: &str) -> String {
    // TODO: implement — join sites with commas, join param_codes with commas,
    // append as query params along with format=json, siteStatus=active,
    // and the period param.
    let _ = (sites, param_codes, period);
    unimplemented!("build_iv_url: construct query string from params")
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
    // TODO: implement with serde_json.
    //
    // Key parsing notes:
    //   - Measurement values arrive as JSON *strings*, not numbers.
    //     Parse with str::parse::<f64>().
    //   - Check parsed value against noDataValue (-999999.0) and return
    //     NwisError::NoDataAvailable if all series are missing.
    //   - The qualifier is in values[0].value[N].qualifiers[0] — a string
    //     like "P" or "A". Default to "P" if absent.
    //   - Only use the most recent value (last entry in the value array,
    //     or index 0 if the array has one entry — USGS sorts ascending).
    let _ = json;
    unimplemented!("parse_iv_response: deserialize WaterML JSON envelope")
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
        let url = build_iv_url(&["05568500"], &["00060", "00065"], "PT3H");
        assert!(
            url.contains("waterservices.usgs.gov/nwis/iv/"),
            "must target the IV endpoint, got: {}",
            url
        );
        assert!(url.contains("format=json"), "must request JSON format");
    }

    #[test]
    fn test_build_url_includes_all_params() {
        let url = build_iv_url(&["05568500"], &["00060", "00065"], "PT3H");
        assert!(url.contains("05568500"), "must include site code");
        assert!(url.contains("00060"), "must include discharge param");
        assert!(url.contains("00065"), "must include stage param");
        assert!(url.contains("PT3H"), "must include ISO 8601 period");
        assert!(url.contains("siteStatus=active"), "should filter to active sites");
    }

    #[test]
    fn test_build_url_with_all_peoria_basin_sites() {
        let sites = all_site_codes();
        let url = build_iv_url(&sites, &[PARAM_DISCHARGE, PARAM_STAGE], "PT1H");
        for site in &sites {
            assert!(url.contains(site), "URL must include site {}", site);
        }
    }

    #[test]
    fn test_build_url_uses_comma_separated_sites() {
        let url = build_iv_url(&["05568500", "05567500"], &["00060"], "PT1H");
        // USGS expects a single comma-separated `sites` param, not repeated params.
        assert!(
            url.contains("05568500,05567500") || url.contains("sites=05568500&sites=05567500"),
            "sites should be comma-separated, got: {}",
            url
        );
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

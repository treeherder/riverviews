///the JSON fixture strings, cfg(test) gated
/// 
/// /// Test fixtures: representative JSON payloads from the USGS IV API.
///
/// These fixtures are structurally complete but truncated to the minimum
/// needed to exercise the parser. They reflect the real WaterML-as-JSON
/// envelope returned by:
///   https://waterservices.usgs.gov/nwis/iv/?format=json&...
///
/// USGS IV response shape:
///   response.value.timeSeries[]
///     .sourceInfo.siteCode[0].value  — site number (string)
///     .sourceInfo.siteName
///     .variable.variableCode[0].value — parameter code (string)
///     .variable.unit.unitCode
///     .variable.noDataValue          — sentinel for missing data (-999999)
///     .values[0].value[]
///       .value     — the measurement as a STRING (not a number)
///       .dateTime  — ISO 8601 with offset
///       .qualifiers[] — e.g. ["P"] or ["A"]
///
/// Note: measurement values are always JSON strings in the USGS response,
/// even though they represent numbers. Parsers must handle this.

/// Single site (Kingston Mines 05568500) with both discharge and stage.
/// Stage of 18.42 ft is above flood stage (16.0) but below moderate (20.0).
#[cfg(test)]
pub(crate) fn fixture_kingston_mines_json() -> &'static str {
    r#"{
      "value": {
        "timeSeries": [
          {
            "sourceInfo": {
              "siteName": "Illinois River at Kingston Mines, IL",
              "siteCode": [{ "value": "05568500", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.5614, "longitude": -89.9956 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00060", "network": "NWIS" }],
              "variableName": "Streamflow, ft&#179;/s",
              "unit": { "unitCode": "ft3/s" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "42300", "qualifiers": ["P"], "dateTime": "2024-05-01T12:00:00.000-05:00" }
              ],
              "qualifier": [{ "qualifierCode": "P", "qualifierDescription": "Provisional data subject to revision." }]
            }]
          },
          {
            "sourceInfo": {
              "siteName": "Illinois River at Kingston Mines, IL",
              "siteCode": [{ "value": "05568500", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.5614, "longitude": -89.9956 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00065", "network": "NWIS" }],
              "variableName": "Gage height, ft",
              "unit": { "unitCode": "ft" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "18.42", "qualifiers": ["P"], "dateTime": "2024-05-01T12:00:00.000-05:00" }
              ],
              "qualifier": [{ "qualifierCode": "P", "qualifierDescription": "Provisional data subject to revision." }]
            }]
          }
        ]
      }
    }"#
}

/// Two sites in one response: Peoria pool gauge (stage only) + Chillicothe
/// (discharge only). Tests multi-site parsing and sparse parameter coverage.
#[cfg(test)]
pub(crate) fn fixture_multi_site_json() -> &'static str {
    r#"{
      "value": {
        "timeSeries": [
          {
            "sourceInfo": {
              "siteName": "Illinois River at Peoria, IL",
              "siteCode": [{ "value": "05567500", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.6939, "longitude": -89.5898 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00065", "network": "NWIS" }],
              "variableName": "Gage height, ft",
              "unit": { "unitCode": "ft" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "14.85", "qualifiers": ["P"], "dateTime": "2024-05-01T12:00:00.000-05:00" }
              ],
              "qualifier": []
            }]
          },
          {
            "sourceInfo": {
              "siteName": "Illinois River at Chillicothe, IL",
              "siteCode": [{ "value": "05568000", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.9200, "longitude": -89.4854 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00060", "network": "NWIS" }],
              "variableName": "Streamflow, ft&#179;/s",
              "unit": { "unitCode": "ft3/s" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "39100", "qualifiers": ["P"], "dateTime": "2024-05-01T11:45:00.000-05:00" }
              ],
              "qualifier": []
            }]
          }
        ]
      }
    }"#
}

/// Mackinaw River gauge with an empty value array — simulates sensor outage
/// or a data gap. Parser should return NoDataAvailable.
#[cfg(test)]
pub(crate) fn fixture_empty_value_array_json() -> &'static str {
    r#"{
      "value": {
        "timeSeries": [
          {
            "sourceInfo": {
              "siteName": "Mackinaw River near Green Valley, IL",
              "siteCode": [{ "value": "05568580", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.7050, "longitude": -89.6480 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00060", "network": "NWIS" }],
              "variableName": "Streamflow, ft&#179;/s",
              "unit": { "unitCode": "ft3/s" },
              "noDataValue": -999999.0
            },
            "values": [{ "value": [], "qualifier": [] }]
          }
        ]
      }
    }"#
}

/// Henry gauge with the USGS sentinel value -999999 — a timestamp is present
/// but the measurement is explicitly missing. Parser must treat this as
/// NoDataAvailable, not as a valid reading of -999999 cfs.
#[cfg(test)]
pub(crate) fn fixture_sentinel_no_data_json() -> &'static str {
    r#"{
      "value": {
        "timeSeries": [
          {
            "sourceInfo": {
              "siteName": "Illinois River at Henry, IL",
              "siteCode": [{ "value": "05557000", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 41.1120, "longitude": -89.3540 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00060", "network": "NWIS" }],
              "variableName": "Streamflow, ft&#179;/s",
              "unit": { "unitCode": "ft3/s" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "-999999", "qualifiers": ["P"], "dateTime": "2024-05-01T12:00:00.000-05:00" }
              ],
              "qualifier": []
            }]
          }
        ]
      }
    }"#
}

/// Kingston Mines with qualifier "A" (approved/reviewed) rather than
/// "P" (provisional). Tests that qualifier parsing handles both values.
#[cfg(test)]
pub(crate) fn fixture_approved_qualifier_json() -> &'static str {
    r#"{
      "value": {
        "timeSeries": [
          {
            "sourceInfo": {
              "siteName": "Illinois River at Kingston Mines, IL",
              "siteCode": [{ "value": "05568500", "network": "NWIS", "agencyCode": "USGS" }],
              "geoLocation": {
                "geogLocation": { "srs": "EPSG:4326", "latitude": 40.5614, "longitude": -89.9956 }
              }
            },
            "variable": {
              "variableCode": [{ "value": "00060", "network": "NWIS" }],
              "variableName": "Streamflow, ft&#179;/s",
              "unit": { "unitCode": "ft3/s" },
              "noDataValue": -999999.0
            },
            "values": [{
              "value": [
                { "value": "38700", "qualifiers": ["A"], "dateTime": "2023-05-01T12:00:00.000-05:00" }
              ],
              "qualifier": [{ "qualifierCode": "A", "qualifierDescription": "Approved for publication -- Processing and review completed." }]
            }]
          }
        ]
      }
    }"#
}
